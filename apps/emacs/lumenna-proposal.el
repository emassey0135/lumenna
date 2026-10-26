;;; lumenna-proposal.el --- Replan from Now and Plan the Day -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Elijah Massey
;; SPDX-License-Identifier: MPL-2.0

;;; Commentary:

;; The core proposes, and the person checks what to take: Replan from Now
;; (the rest of today worked out again) and Plan the Day (each work block
;; given its most urgent tasks).  The app's proposal screen, drawn in a buffer
;; of checkable lines beside the day it was asked from, each the core's
;; sentence, all checked to begin with; SPC turns one off or on, C-c C-c makes
;; what is checked.  Nothing changes until then.
;;
;; A change taken alone can leave the day crowded, so applying shows the
;; core's next proposal, worked out around what was left unchecked, in place of
;; the last; the buffer closes when nothing more is proposed.  Every word on
;; the screen is the core's.

;;; Code:

(require 'lumenna)

(defvar lumenna-proposal--now nil
  "For tests: a function of no arguments giving the moment to propose against,
YYYY-MM-DDTHH:MM local, in place of now.")

(define-derived-mode lumenna-proposal-mode lumenna-list-mode "Lumenna Proposal"
  "A proposal of the core's, a change per line, each checked to begin with.
SPC or RET turns one off or on;
\\<lumenna-proposal-mode-map>\\[lumenna-proposal-apply] makes the ones checked; \\[lumenna-proposal-cancel] closes, making nothing.

\\{lumenna-proposal-mode-map}"
  (setq-local lumenna--draw #'lumenna-proposal--rows)
  (setq-local lumenna--activate #'lumenna-proposal--activate))

(defun lumenna-proposal--view ()
  "The proposal this buffer's screen shows."
  (plist-get lumenna--view :proposal))

(defun lumenna-proposal--rows ()
  "The heading and lines of the proposal shown."
  (let* ((proposal (lumenna-proposal--view))
         (words (plist-get proposal :words))
         (changes (append (plist-get proposal :changes) nil))
         (unfit (append (plist-get proposal :unfit) nil))
         (others (append (plist-get proposal :others) nil)))
    (setq-local lumenna--empty nil)
    (if (null proposal)
        (list "Nothing proposed")
      (cons (lumenna--heading (plist-get words :title) (plist-get proposal :announcement))
            (append
             (when-let* ((refused (plist-get proposal :refusal)))
               (list (list :key "refused" :role "error" :text refused :face 'lumenna-overdue)))
             (if changes
                 (seq-map-indexed
                  (lambda (change index)
                    (let ((checked (lumenna--true (plist-get change :checked))))
                      ;; The reason is the change's description, so it follows its name and state.
                      (list :key (format "item %d" index) :role "change" :index index :on checked
                            :text (string-join (delq nil (list (plist-get change :sentence)
                                                               (plist-get change :state)
                                                               (plist-get change :reason)))
                                               ", ")
                            :face (unless checked 'lumenna-quiet))))
                  changes)
               (list (list :key "nothing" :role "nothing" :text (plist-get words :nothing) :face 'lumenna-quiet)))
             (when unfit
               (cons (list :key "unfit" :role "heading" :text (plist-get words :unfit) :face 'lumenna-heading)
                     (mapcar (lambda (line) (list :key (concat "unfit: " line) :role "unfit" :depth 1 :text line))
                             unfit)))
             ;; What becomes of a change left unchecked, beside the button that makes the rest.
             (when changes
               (list (list :key "apply" :role "button" :button 'apply
                           :text (format "%s. %s" (plist-get words :apply) (plist-get words :hint)))))
             ;; Other options, each proposing again in place; none, no heading.
             (when others
               (cons (list :key "others" :role "heading" :text (plist-get words :others) :face 'lumenna-heading)
                     (seq-map-indexed (lambda (other index)
                                        (list :key (format "other %d" index) :role "button" :button 'other
                                              :index index :depth 1 :text other))
                                      others)))
             (list (list :key "cancel" :role "button" :button 'cancel :text (plist-get words :cancel))))))))

(defun lumenna-proposal--open ()
  "Show the proposal the app opened on this buffer's screen, beside it.
Run after what the person asked of the app; a proposal already shown stays."
  (when-let* ((proposal (lumenna-proposal--view)))
    (when-let* ((shown (and (not (derived-mode-p 'lumenna-proposal-mode))
                            (seq-find (lambda (b) (with-current-buffer b (derived-mode-p 'lumenna-proposal-mode)))
                                      (lumenna--buffers-of lumenna--screen)))))
      ;; Asked for again while it is open: the buffer that shows it comes back.
      (pop-to-buffer-same-window shown))
    (unless (derived-mode-p 'lumenna-proposal-mode)
      (let ((screen lumenna--screen)
            (view lumenna--view)
            (lines lumenna--lines)
            (buffer (get-buffer-create (format "*Lumenna: %s*" (plist-get (plist-get proposal :words) :title)))))
        (with-current-buffer buffer
          (lumenna-proposal-mode)
          (lumenna--attach screen)
          (setq lumenna--view view lumenna--lines lines)
          (lumenna--redraw)
          (goto-char (point-min))
          (forward-line 1))
        (pop-to-buffer-same-window buffer)))))

(add-hook 'lumenna-followed-hook #'lumenna-proposal--open)

(defun lumenna-proposal--send (event)
  "Send EVENT, one of the proposal screen's, and follow what it does.
Once the app closes the screen, this buffer goes."
  (lumenna-send (list :Proposal event))
  (lumenna--follow)
  (unless (lumenna-proposal--view)
    (let ((buffer (current-buffer)))
      (quit-window nil (get-buffer-window buffer))
      (kill-buffer buffer))))

(defun lumenna--propose (kind)
  "Do the day's own action of KIND, a proposal: from the moment pinned for tests."
  (when lumenna-proposal--now
    (lumenna-send (list :PlanFrom (funcall lumenna-proposal--now))))
  (lumenna--day-act kind))

(declare-function lumenna--day-act "lumenna-day" (kind))
(declare-function lumenna-today "lumenna-day" ())

;;;###autoload
(defun lumenna-replan (&optional event)
  "Replan the rest of today: the core's proposal, to check and make.
From anywhere, as an overrun warning's button opens it: then EVENT is that
button's, the app's, which dismisses the warning once something is replanned."
  (interactive)
  (unless (derived-mode-p 'lumenna-day-mode) (lumenna-today))
  (when lumenna-proposal--now
    (lumenna-send (list :PlanFrom (funcall lumenna-proposal--now))))
  (lumenna-send (or event (list :Proposal "Replan")))
  (lumenna--follow))

;;;###autoload
(defun lumenna-plan-the-day (&optional date)
  "Plan DATE, a date phrase, else the day shown or today.
Each work block is given its most urgent tasks."
  (interactive)
  (unless (derived-mode-p 'lumenna-day-mode) (lumenna-today))
  (when date (lumenna-day-show date))
  (lumenna--propose "plan_day"))

(declare-function lumenna-day-show "lumenna-day" (date))

(defun lumenna-proposal--activate (row)
  "RET on ROW: turn a change off or on, or press the button it is."
  (pcase (plist-get row :button)
    ('apply (lumenna-proposal-apply))
    ('cancel (lumenna-proposal-cancel))
    ('other (lumenna-proposal-take-other))
    ((guard (equal (plist-get row :role) "change")) (lumenna-proposal-toggle))
    (_ (message "%s" (plist-get row :text)))))

(defun lumenna-proposal-take-other ()
  "Take the other option at point: the core's proposal for it, in place."
  (interactive)
  (let ((row (lumenna-row)))
    (unless (eq (plist-get row :button) 'other) (user-error "No other option on this line"))
    (lumenna-proposal--send (list :Take (plist-get row :index)))
    (goto-char (point-min))
    (forward-line 1)))

(defun lumenna-proposal-toggle ()
  "Turn the change at point off, or back on, and say which it is now."
  (interactive)
  (let ((row (lumenna-row)))
    (unless (equal (plist-get row :role) "change")
      (user-error "%s" (plist-get row :text)))
    (let ((on (not (plist-get row :on))))
      (lumenna-proposal--send (list :Check (list :index (plist-get row :index) :on (if on t :json-false))))
      ;; Its state as the app now has it, in the app's words.
      (when-let* ((proposal (lumenna-proposal--view))
                  (change (nth (plist-get row :index) (append (plist-get proposal :changes) nil))))
        (funcall lumenna-announce-function
                 (format "%s, %s" (plist-get (plist-get proposal :words) :item) (plist-get change :state))
                 nil)))))

(defun lumenna-proposal-apply ()
  "Make the changes checked.
The core proposes again around what was left unchecked; that is shown next,
and the buffer closes once nothing more is proposed.  A proposal the day has
moved past is refused: the core's sentence is shown, over a new proposal."
  (interactive)
  (unless (plist-get (lumenna-proposal--view) :changes)
    (user-error "%s" (plist-get (plist-get (lumenna-proposal--view) :words) :nothing)))
  (setq lumenna--acting (if (equal (plist-get (lumenna-proposal--view) :kind) "replan") "day/replan" "day/plan_day"))
  (lumenna-proposal--send "Apply")
  (when (derived-mode-p 'lumenna-proposal-mode)
    (goto-char (point-min))
    (forward-line 1)))

(defun lumenna-proposal-cancel ()
  "Close the proposal, making nothing."
  (interactive)
  (lumenna-proposal--send "Close"))

(lumenna-define-keys lumenna-proposal-mode
  ("The proposal"
   ("SPC" "Check or uncheck the change at point" lumenna-proposal-toggle)
   ("x" "Check or uncheck the change at point" lumenna-proposal-toggle)
   ("RET" "Check or uncheck a change, or press a button" lumenna-activate)
   ("o" "Take the other option at point" lumenna-proposal-take-other)
   ("C-c C-c" "Make the changes checked" lumenna-proposal-apply)
   ("C-c C-k" "Cancel, making nothing" lumenna-proposal-cancel)
   ("q" "Cancel, making nothing" lumenna-proposal-cancel)))

(provide 'lumenna-proposal)

;;; lumenna-proposal.el ends here
