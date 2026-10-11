;;; lumenna-proposal.el --- Replan from Now and Plan the Day -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Elijah Massey
;; SPDX-License-Identifier: MPL-2.0

;;; Commentary:

;; The core proposes, and the person checks what to take: Replan from Now
;; (the rest of today worked out again) and Plan the Day (each work block
;; given its most urgent tasks).  A buffer of checkable lines, each the core's
;; sentence, all checked to begin with; SPC turns one off or on, C-c C-c makes
;; what is checked.  Nothing changes until then.
;;
;; A change taken alone can leave the day crowded, so applying hands back the
;; core's next proposal, worked out around what was left unchecked, and the
;; buffer shows it in place of the last; it closes when nothing more is
;; proposed.  Every word on the screen is the core's (`form.proposal').

;;; Code:

(require 'lumenna)

(defvar lumenna-proposal--now nil
  "For tests: a function of no arguments giving now, a local date and time.
Nil asks the core for now.")

(defvar-local lumenna-proposal--kind nil
  "Which proposal this buffer shows: \"replan\" or \"plan_day\".")

(defvar-local lumenna-proposal--date nil
  "The day Plan the Day plans, ISO; nil for today.")

(defvar-local lumenna-proposal--proposal nil
  "The core's proposal shown: a `Replan' or a `DayPlan'.")

(defvar-local lumenna-proposal--checked nil
  "Whether each of the proposal's items is checked, in its order.")

(defvar-local lumenna-proposal--refused nil
  "The core's sentence when the last apply was refused, shown over the list.")

(define-derived-mode lumenna-proposal-mode lumenna-list-mode "Lumenna Proposal"
  "A proposal of the core's, a change per line, each checked to begin with.
SPC or RET turns one off or on;
\\<lumenna-proposal-mode-map>\\[lumenna-proposal-apply] makes the ones checked; \\[lumenna-proposal-cancel] closes, making nothing.

\\{lumenna-proposal-mode-map}"
  (setq-local lumenna--describe #'lumenna-proposal--describe)
  (setq-local lumenna--activate #'lumenna-proposal--activate))

(defun lumenna-proposal--replan-p ()
  "Whether this buffer is Replan from Now's."
  (equal lumenna-proposal--kind "replan"))

(defun lumenna-proposal--words (&optional kind)
  "The words of KIND's screen, or this buffer's: title, item, apply, cancel, hint."
  (lumenna-words "form.proposal" :kind (or kind lumenna-proposal--kind)))

(defun lumenna-proposal--items (&optional proposal)
  "PROPOSAL's items, or the shown one's: a replan's changes, a day plan's sittings."
  (let ((proposal (or proposal lumenna-proposal--proposal)))
    (append (or (plist-get proposal :changes) (plist-get proposal :sittings)) nil)))

(defun lumenna-proposal--clock ()
  "The clock the core is to say times in: Emacs's own, as `lumenna-time' says them."
  (if display-time-24hr-format "24-hour" "12-hour"))

(defun lumenna-proposal--ask (kind date &optional pins)
  "What the core proposes for KIND, on DATE for a day plan, keeping to PINS."
  (let ((now (and lumenna-proposal--now (funcall lumenna-proposal--now))))
    (if (equal kind "replan")
        (lumenna-call "replan" :clock (lumenna-proposal--clock) :pins pins :now now)
      (lumenna-call "day_plan" :date date :clock (lumenna-proposal--clock) :pins pins :now now))))

(defun lumenna-proposal--open (kind date)
  "Show what the core proposes for KIND, on DATE for a day plan, and say it."
  (let* ((proposal (lumenna-proposal--ask kind date))
         (title (plist-get (lumenna-proposal--words kind) :title)))
    (lumenna--show-list (format "*Lumenna: %s*" title) #'lumenna-proposal-mode #'lumenna-proposal--rows
                        'lumenna-proposal--kind kind
                        'lumenna-proposal--date date
                        'lumenna-proposal--proposal proposal
                        'lumenna-proposal--checked (make-list (length (lumenna-proposal--items proposal)) t)
                        'lumenna-proposal--refused nil)
    (lumenna-proposal--first-line)
    ;; The whole proposal in one sentence: for someone who cannot glance at a
    ;; timeline, that is the check.
    (lumenna-say proposal)))

;;;###autoload
(defun lumenna-replan ()
  "Replan the rest of today: the core's proposal, to check and make."
  (interactive)
  (lumenna-proposal--open "replan" nil))

;;;###autoload
(defun lumenna-plan-the-day (&optional date)
  "Plan DATE, ISO or a date phrase, else today: each work block given its tasks."
  (interactive)
  (lumenna-proposal--open "plan_day" date))

(lumenna-define-form "day" "replan" (lambda (_action _row) (lumenna-replan)))
(lumenna-define-form "day" "plan_day" (lambda (action _row) (lumenna-plan-the-day (plist-get action :target))))

(defun lumenna-proposal--rows ()
  "The heading and lines of the proposal shown."
  (let* ((words (lumenna-proposal--words))
         (proposal lumenna-proposal--proposal)
         (items (lumenna-proposal--items))
         (unfit (append (or (plist-get proposal :unplaceable) (plist-get proposal :unfit)) nil)))
    (cons (format "%s. %s" (plist-get words :title) (plist-get proposal :announcement))
          (append
           (when lumenna-proposal--refused
             (list (list :key "refused" :role "error" :title lumenna-proposal--refused :face 'lumenna-overdue)))
           (if items
               (seq-map-indexed
                (lambda (item index)
                  (let ((checked (nth index lumenna-proposal--checked)))
                    (list :key (format "item %d" index) :role "change" :index index :item item
                          :title (plist-get item :sentence) :on checked :reason (plist-get item :reason)
                          :face (unless checked 'lumenna-quiet))))
                items)
             (list (list :key "nothing" :role "nothing" :title (plist-get words :nothing) :face 'lumenna-quiet)))
           (when unfit
             (cons (list :key "unfit" :role "heading" :title (plist-get words :unfit) :face 'lumenna-heading)
                   (mapcar (lambda (line) (list :key (concat "unfit: " line) :role "unfit" :depth 1 :title line))
                           unfit)))
           ;; What becomes of a change left unchecked, beside the button that makes the rest.
           (when items
             (list (list :key "apply" :role "button" :button 'apply
                         :title (format "%s. %s" (plist-get words :apply) (plist-get words :hint)))))
           ;; Other options, each asking again with its own pins; none, no heading.
           (when-let* ((others (append (plist-get proposal :alternatives) nil)))
             (cons (list :key "others" :role "heading" :title (plist-get words :others) :face 'lumenna-heading)
                   (seq-map-indexed (lambda (other index)
                                      (list :key (format "other %d" index) :role "button" :button 'other
                                            :depth 1 :other other :title (plist-get other :sentence)))
                                    others)))
           (list (list :key "cancel" :role "button" :button 'cancel :title (plist-get words :cancel)))))))

(defun lumenna-proposal--describe (row)
  "ROW as its line: a change's sentence, whether it is checked, then why.
The reason is the change's description, so it follows its name and state."
  (if (equal (plist-get row :role) "change")
      (string-join (delq nil (list (plist-get row :title)
                                   (if (plist-get row :on) "checked" "not checked")
                                   (plist-get row :reason)))
                   ", ")
    (plist-get row :title)))

(defun lumenna-proposal--first-line ()
  "Point on the first line under the heading."
  (goto-char (point-min))
  (forward-line 1))

(defun lumenna-proposal--activate (row)
  "RET on ROW: turn a change off or on, or press the button it is."
  (pcase (plist-get row :button)
    ('apply (lumenna-proposal-apply))
    ('cancel (lumenna-proposal-cancel))
    ('other (lumenna-proposal-take-other))
    ((guard (equal (plist-get row :role) "change")) (lumenna-proposal-toggle))
    (_ (message "%s" (plist-get row :title)))))

(defun lumenna-proposal-take-other ()
  "Take the other option at point: the core's proposal for it, shown and said."
  (interactive)
  (let ((other (or (plist-get (lumenna-row) :other) (user-error "No other option on this line"))))
    (let ((again (lumenna-proposal--ask lumenna-proposal--kind lumenna-proposal--date (plist-get other :pins))))
      (lumenna-proposal--show again)
      (lumenna-say again))))

(defun lumenna-proposal-toggle ()
  "Turn the change at point off, or back on, and say which it is now."
  (interactive)
  (let ((row (lumenna-row)))
    (unless (equal (plist-get row :role) "change")
      (user-error "%s" (plist-get row :title)))
    (let ((index (plist-get row :index)))
      (setf (nth index lumenna-proposal--checked) (not (nth index lumenna-proposal--checked)))
      (lumenna-refresh)
      (funcall lumenna-announce-function
               (format "%s, %s" (plist-get (lumenna-proposal--words) :item)
                       (if (nth index lumenna-proposal--checked) "checked" "not checked"))
               nil))))

(defun lumenna-proposal--show (proposal &optional refused)
  "Show PROPOSAL in place of the last, every item checked; REFUSED over it."
  (setq lumenna-proposal--proposal proposal
        lumenna-proposal--checked (make-list (length (lumenna-proposal--items proposal)) t)
        lumenna-proposal--refused refused)
  (lumenna-refresh)
  (lumenna-proposal--first-line))

(defun lumenna-proposal-apply ()
  "Make the changes checked.
The core proposes again around what was left unchecked; that is shown next,
and the buffer closes once nothing more is proposed.  A proposal the day has
moved past is refused: the core's sentence is shown, over a new proposal."
  (interactive)
  (unless (lumenna-proposal--items)
    (user-error "%s" (plist-get (lumenna-proposal--words) :nothing)))
  (let* ((proposal lumenna-proposal--proposal)
         (accepted (vconcat (seq-filter #'identity
                                        (seq-mapn (lambda (item on) (and on item))
                                                  (lumenna-proposal--items) lumenna-proposal--checked))))
         (now (and lumenna-proposal--now (funcall lumenna-proposal--now)))
         (buffer (current-buffer))
         (result (condition-case err
                     (if (lumenna-proposal--replan-p)
                         (lumenna-call "replan.apply" :proposal proposal :accepted accepted :now now)
                       (lumenna-call "day_plan.apply" :plan proposal :accepted accepted :now now))
                   (user-error
                    (let ((again (lumenna-proposal--ask lumenna-proposal--kind lumenna-proposal--date
                                                        (plist-get proposal :pins))))
                      (lumenna-proposal--show again (cadr err))
                      (funcall lumenna-announce-function (cadr err) (list (plist-get again :announcement)))
                      nil)))))
    (when result
      (let ((next (plist-get result :next)))
        (if (lumenna-proposal--items next)
            (lumenna-proposal--show next)
          (quit-window t (get-buffer-window buffer))
          (when (buffer-live-p buffer) (kill-buffer buffer))))
      (lumenna--wrote (format "day/%s" (if (lumenna-proposal--replan-p) "replan" "plan_day")) result))))

(defun lumenna-proposal-cancel ()
  "Close the proposal, making nothing."
  (interactive)
  (quit-window t))

(defun lumenna-proposal-again ()
  "Ask the core again, keeping to what was decided, every item checked."
  (interactive)
  (let ((again (lumenna-proposal--ask lumenna-proposal--kind lumenna-proposal--date
                                      (plist-get lumenna-proposal--proposal :pins))))
    (lumenna-proposal--show again)
    (lumenna-say again)))

(lumenna-define-keys lumenna-proposal-mode
  ("The proposal"
   ("SPC" "Check or uncheck the change at point" lumenna-proposal-toggle)
   ("x" "Check or uncheck the change at point" lumenna-proposal-toggle)
   ("RET" "Check or uncheck a change, or press a button" lumenna-activate)
   ("o" "Take the other option at point" lumenna-proposal-take-other)
   ("C-c C-c" "Make the changes checked" lumenna-proposal-apply)
   ("C-c C-k" "Cancel, making nothing" lumenna-proposal-cancel)
   ("q" "Cancel, making nothing" lumenna-proposal-cancel)
   ("g" "Propose again" lumenna-proposal-again)))

(provide 'lumenna-proposal)

;;; lumenna-proposal.el ends here
