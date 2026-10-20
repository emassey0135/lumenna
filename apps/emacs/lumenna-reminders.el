;;; lumenna-reminders.el --- Lumenna's reminders, given while Emacs runs -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Elijah Massey
;; SPDX-License-Identifier: MPL-2.0

;;; Commentary:

;; Reminders, given while Emacs runs: every twenty seconds, and on coming
;; back to Emacs, `reminder.deliver' hands over what came due, and each is said
;; through `lumenna-announce-function' -- the echo area, or Emacsvox's
;; notification lane -- and shown as a desktop notification where D-Bus has a
;; notification service, with the core's Dismiss and Snooze as its buttons.
;; Only while no other process on this device gives reminders: the Mac app,
;; the GTK app or `lum notify' does it better, and the server tells this
;; connection when one does.
;;
;; What came due and was not dealt with stays in M-x lumenna-reminders-due,
;; where d dismisses one, z snoozes it, and c carries forward what a block's
;; end left short, where its reminder offers to.  A task's or block's own
;; reminders are its Reminders action (R): a list of them, a to add one by a
;; phrase the core reads ("5m before start"), d to remove one.  When each
;; fires, what it says and the buttons' words are all the core's.

;;; Code:

(require 'lumenna)
;; Only where Emacs was built with D-Bus: elsewhere loading it signals.
(when (featurep 'dbusbind)
  (require 'notifications nil t))

(declare-function notifications-notify "notifications" (&rest params))
(declare-function lumenna-task-show "lumenna-tasks" (id))
(declare-function lumenna-today "lumenna-day" ())
(declare-function lumenna-today "lumenna-day" ())
(declare-function lumenna--task "lumenna-tasks" (&optional id))
(declare-function dbus-ping "dbus" (bus service &optional timeout))
(declare-function lumenna-replan "lumenna-proposal" ())

(defcustom lumenna-reminders-start-automatically t
  "Whether reminders start being given once Lumenna first connects.
They can also be started from the init file with (lumenna-reminders-mode 1)."
  :type 'boolean
  :group 'lumenna)

(defcustom lumenna-reminders-desktop t
  "Whether a reminder is shown as a desktop notification too, where D-Bus has one.
The echo area, and Emacsvox or Emacspeak, say it either way."
  :type 'boolean
  :group 'lumenna)

(defconst lumenna-reminders-interval 20
  "How often, in seconds, to ask what came due.")

(defconst lumenna-reminders-waiting-interval 5
  "How often, in seconds, to ask while another process gives reminders.
Soon after it stops, this one takes over.")

(defvar lumenna-reminders--every lumenna-reminders-interval
  "The timer's interval now: shorter while another process gives reminders.")

(defvar lumenna-reminders--now nil
  "For tests: a function of no arguments giving the moment to deliver at, RFC 3339.
Nil asks the core for now.")

(defvar lumenna-reminders--timer nil
  "The timer giving reminders, while `lumenna-reminders-mode' is on.")

(defvar lumenna-reminders--asking nil
  "Whether a delivery has been asked for and not yet answered.")

(defvar lumenna-reminders--waiting nil
  "What came due and was not dealt with yet, oldest first.
Each is the core's `DueReminder' plist, or for what was missed while nothing
ran, a plist with `:summary' and `:missed', the missed reminders.")

;;;; Giving them

(defun lumenna-reminders--desktop-p ()
  "Whether a desktop notification can be shown here."
  (and lumenna-reminders-desktop (featurep 'notifications) (fboundp 'notifications-notify)
       (featurep 'dbusbind)
       (ignore-errors (dbus-ping :session "org.freedesktop.Notifications" 500))))

(defun lumenna-reminders--key (reminder)
  "What tells REMINDER apart from every other firing."
  (if (plist-get reminder :summary)
      (cons 'missed (mapcar #'lumenna-reminders--key (plist-get reminder :missed)))
    (cons (plist-get reminder :key) (plist-get reminder :occurrence))))

(defun lumenna-reminders--say (text)
  "Say TEXT, a reminder, through the speech layers, with its sound."
  (run-hook-with-args 'lumenna-changed-functions "reminder/due" nil)
  (funcall lumenna-announce-function text nil))

(defun lumenna-reminders--notify (reminder)
  "Show REMINDER as a desktop notification, its buttons the core's words."
  (when (lumenna-reminders--desktop-p)
    (let* ((first (if (plist-get reminder :summary) (car (plist-get reminder :missed)) reminder))
           (title (if (plist-get reminder :summary) "Lumenna" (plist-get reminder :title)))
           (body (or (plist-get reminder :summary) (plist-get reminder :text))))
      (ignore-errors
        (notifications-notify
         :title title :body body :app-name "Lumenna"
         :actions (append (list "dismiss" (plist-get first :dismiss))
                          (unless (plist-get reminder :summary)
                            (append (list "snooze" (plist-get first :snooze))
                                    ;; At a block's end with work left short, the text ends
                                    ;; in the question this button answers.
                                    (when (lumenna-reminders--carry-p reminder)
                                      (list "carry" (plist-get reminder :carry)))
                                    ;; A block's work outrunning its time: Replan from Now.
                                    (when (lumenna-reminders--replan-p reminder)
                                      (list "replan" (plist-get reminder :replan)))
                                    (list "default" (plist-get first :title)))))
         :on-action (lambda (_id action)
                      (pcase action
                        ("dismiss" (lumenna-reminders--deal reminder "reminder.dismiss"))
                        ("snooze" (lumenna-reminders--deal reminder "reminder.snooze"))
                        ("carry" (lumenna-reminders--carry reminder))
                        ("replan" (lumenna-replan))
                        ("default" (lumenna-reminders--open reminder)))))))))

(defun lumenna-reminders--give (found)
  "Give what FOUND, the core's `DueReminders', says came due."
  (let ((summary (plist-get found :missed_summary))
        (missed (append (plist-get found :missed) nil)))
    (when (and summary (not (string-empty-p summary)))
      (let ((entry (list :summary summary :missed missed)))
        (setq lumenna-reminders--waiting (append lumenna-reminders--waiting (list entry)))
        (lumenna-reminders--say summary)
        (lumenna-reminders--notify entry))))
  (dolist (reminder (append (plist-get found :due) nil))
    (setq lumenna-reminders--waiting (append lumenna-reminders--waiting (list reminder)))
    (lumenna-reminders--say (plist-get reminder :text))
    (lumenna-reminders--notify reminder))
  (when (plist-get found :due)
    (lumenna-reminders--redraw)))

(defun lumenna-reminders-deliver ()
  "Ask what came due, and give it.  In the background; Emacs stays free."
  (interactive)
  (unless lumenna-reminders--asking
    (setq lumenna-reminders--asking t)
    (condition-case err
        (jsonrpc-async-request
         (lumenna--connection) 'reminder.deliver
         (lumenna--params (list :now (and lumenna-reminders--now (funcall lumenna-reminders--now))))
         :success-fn (lambda (found)
                       (setq lumenna-reminders--asking nil)
                       (lumenna-reminders--pace (lumenna--true (plist-get found :elsewhere)))
                       (lumenna-reminders--give found))
         :error-fn (lambda (err)
                     (setq lumenna-reminders--asking nil)
                     (message "Lumenna reminders: %s" (plist-get err :message)))
         :timeout-fn (lambda () (setq lumenna-reminders--asking nil))
         :timeout 30)
      (error (setq lumenna-reminders--asking nil)
             (message "Lumenna reminders: %s" (error-message-string err))))))

(defun lumenna-reminders--start (every)
  "Ask what came due every EVERY seconds from now."
  (when lumenna-reminders--timer (cancel-timer lumenna-reminders--timer))
  (setq lumenna-reminders--every every
        lumenna-reminders--timer (run-at-time every every #'lumenna-reminders-deliver)))

(defun lumenna-reminders--pace (elsewhere)
  "Ask more often while ELSEWHERE, another process giving reminders; silently."
  (let ((every (if elsewhere lumenna-reminders-waiting-interval lumenna-reminders-interval)))
    (when (and lumenna-reminders-mode (/= every lumenna-reminders--every))
      (lumenna-reminders--start every))))

(defun lumenna-reminders--focus-changed ()
  "Ask again on coming back to Emacs, as an app does on coming to the front."
  (when (and lumenna-reminders-mode (frame-focus-state))
    (lumenna-reminders-deliver)))

;;;###autoload
(define-minor-mode lumenna-reminders-mode
  "Give Lumenna's reminders while Emacs runs.
Every twenty seconds, and on coming back to Emacs, what came due is said
and shown, unless another process on this device already gives them."
  :global t
  :group 'lumenna
  (when lumenna-reminders--timer
    (cancel-timer lumenna-reminders--timer)
    (setq lumenna-reminders--timer nil))
  (remove-function after-focus-change-function #'lumenna-reminders--focus-changed)
  (when lumenna-reminders-mode
    (lumenna-reminders--start lumenna-reminders-interval)
    (lumenna-reminders-deliver)
    (add-function :after after-focus-change-function #'lumenna-reminders--focus-changed)))

(defun lumenna-reminders--connected ()
  "Start giving reminders on the first connection, if that is wanted."
  (when (and lumenna-reminders-start-automatically (not lumenna-reminders-mode))
    (lumenna-reminders-mode 1)))

(add-hook 'lumenna-connected-hook #'lumenna-reminders--connected)

;;;; Dealing with one

(defun lumenna-reminders--deal (reminder method)
  "Do METHOD, `reminder.dismiss' or `reminder.snooze', to REMINDER, and say it.
A summary of what was missed is dismissed one by one."
  (let ((each (if (plist-get reminder :summary) (plist-get reminder :missed) (list reminder)))
        result)
    (dolist (one each)
      (setq result (lumenna-call method :key (plist-get one :key) :occurrence (plist-get one :occurrence))))
    (setq lumenna-reminders--waiting
          (seq-remove (lambda (r) (equal (lumenna-reminders--key r) (lumenna-reminders--key reminder)))
                      lumenna-reminders--waiting))
    (lumenna-reminders--wrote method result)))

(defun lumenna-reminders--wrote (event result)
  "Say RESULT, what EVENT, a store method, did; the app's screens take it in."
  (run-hook-with-args 'lumenna-changed-functions event result)
  (lumenna-say result)
  (lumenna--tick)
  (lumenna-reminders--redraw))

(defun lumenna-reminders--not-offered (_kind)
  "Why a reminder does not offer an action: it has no button for it."
  "Nothing to do here")

(defun lumenna-reminders--carry-p (reminder)
  "Whether REMINDER offers to carry its block's short work forward."
  (let ((carry (plist-get reminder :carry)))
    (and (stringp carry) (not (string-empty-p carry)))))

(defun lumenna-reminders--replan-p (reminder)
  "Whether REMINDER offers Replan from Now: a block's work outrunning its time."
  (let ((replan (plist-get reminder :replan)))
    (and (stringp replan) (not (string-empty-p replan)))))

(defun lumenna-reminders-replan ()
  "Open Replan from Now, as the reminder at point offers to."
  (interactive)
  (unless (lumenna-reminders--replan-p (lumenna-reminders--at-point))
    (user-error "%s" (lumenna-reminders--not-offered "replan")))
  (lumenna-replan))

(defun lumenna-reminders--carry (reminder)
  "Carry the work REMINDER's block left short forward (`carry_rest'), and say it.
That answers the reminder, so it leaves what came due."
  (let ((result (lumenna-call "carry_rest" :block (format "%s@%s" (plist-get reminder :target)
                                                          (plist-get reminder :occurrence)))))
    (setq lumenna-reminders--waiting
          (seq-remove (lambda (r) (equal (lumenna-reminders--key r) (lumenna-reminders--key reminder)))
                      lumenna-reminders--waiting))
    (lumenna-reminders--wrote "block/carry_rest" result)))

(defun lumenna-reminders--open (reminder)
  "Open what REMINDER is about: its task, or its block's day."
  (let ((first (if (plist-get reminder :summary) (car (plist-get reminder :missed)) reminder)))
    (pcase (plist-get first :kind)
      ("task" (lumenna-task-show (plist-get first :target)))
      (_ (with-current-buffer (lumenna-today)
           (lumenna-send (list :ShowBlock (list :series (plist-get first :target) :date (plist-get first :occurrence))))
           (lumenna--follow))))))

;;;; What came due

(define-derived-mode lumenna-reminders-due-mode lumenna-list-mode "Lumenna Due"
  "What came due and was not dealt with yet.  d dismisses one, z snoozes it.

\\{lumenna-reminders-due-mode-map}"
  (setq-local lumenna--activate #'lumenna-reminders--open))

(defun lumenna-reminders--due-rows ()
  "The reminders waiting, as rows."
  (setq-local lumenna--empty "Nothing has come due")
  (cons (if lumenna-reminders--waiting
            (format "Reminders due, %d" (length lumenna-reminders--waiting))
          "Reminders due")
        (mapcar (lambda (reminder)
                  (list :key (lumenna-reminders--key reminder) :role "reminder" :reminder reminder
                        :title (or (plist-get reminder :summary) (plist-get reminder :text))))
                lumenna-reminders--waiting)))

(defun lumenna-reminders--redraw ()
  "Redraw the list of what came due, if it is shown."
  (when-let* ((buffer (get-buffer "*Lumenna: Reminders due*")))
    (with-current-buffer buffer (lumenna-refresh))))

;;;###autoload
(defun lumenna-reminders-due ()
  "List what came due and was not dealt with: d dismisses one, z snoozes it."
  (interactive)
  (lumenna--show-list "*Lumenna: Reminders due*" #'lumenna-reminders-due-mode #'lumenna-reminders--due-rows))

(defun lumenna-reminders--at-point ()
  "The reminder at point, or a `user-error'."
  (or (plist-get (lumenna-row) :reminder) (user-error "No reminder on this line")))

(defun lumenna-reminders-dismiss ()
  "Dismiss the reminder at point, on every device."
  (interactive)
  (lumenna-reminders--deal (lumenna-reminders--at-point) "reminder.dismiss"))

(defun lumenna-reminders-snooze ()
  "Snooze the reminder at point, on every device, for as long as the core says."
  (interactive)
  (let ((reminder (lumenna-reminders--at-point)))
    (when (plist-get reminder :summary) (user-error "What was missed can be dismissed, not snoozed"))
    (lumenna-reminders--deal reminder "reminder.snooze")))

(defun lumenna-reminders-carry ()
  "Carry what the block of the reminder at point left short forward, as it offers."
  (interactive)
  (let ((reminder (lumenna-reminders--at-point)))
    (unless (lumenna-reminders--carry-p reminder)
      (user-error "%s" (lumenna-reminders--not-offered "carry_rest")))
    (lumenna-reminders--carry reminder)))

;;;; A task's or block's own

;; The app's reminder list, opened by a task's or block's Reminders action, in
;; a buffer of its own beside the list it was asked from.

(define-derived-mode lumenna-reminder-list-mode lumenna-list-mode "Lumenna Reminders"
  "A task's or block's reminders: its own, or the defaults it takes.
a adds one, by a phrase such as \"5m before start\"; d removes one of its own.

\\{lumenna-reminder-list-mode-map}"
  (setq-local lumenna--held :reminders_held)
  (setq-local lumenna--draw #'lumenna-reminders--rows)
  (add-hook 'kill-buffer-hook #'lumenna-reminders--close nil t))

(defun lumenna-reminders--list ()
  "The reminder list this buffer's screen shows."
  (plist-get lumenna--view :reminders))

(defun lumenna-reminders--rows ()
  "The reminders of the list shown, in the core's words."
  (let* ((list (lumenna-reminders--list))
         (lines (append (plist-get lumenna--lines :reminders) nil)))
    (setq-local lumenna--empty nil)
    (if (null list)
        (list "No reminders shown")
      (cons (plist-get list :announcement)
            (cl-mapcar (lambda (reminder line)
                         (list :key (plist-get reminder :id) :id (plist-get reminder :id) :role "reminder"
                               :text (plist-get line :text) :actions (plist-get reminder :actions)))
                       (append (plist-get list :reminders) nil) lines)))))

(defun lumenna-reminders--open-list ()
  "Show the reminder list the app opened on this buffer's screen, beside it."
  (when-let* ((list (lumenna-reminders--list)))
    (unless (or (derived-mode-p 'lumenna-reminder-list-mode)
                (seq-find (lambda (b) (with-current-buffer b (derived-mode-p 'lumenna-reminder-list-mode)))
                          (lumenna--buffers-of lumenna--screen)))
      (let ((screen lumenna--screen) (view lumenna--view) (lines lumenna--lines)
            (buffer (get-buffer-create
                     (format "*Lumenna reminders: %s*"
                             (or (plist-get (plist-get lumenna--acted-on :block) :title)
                                 (plist-get lumenna--acted-on :title)
                                 (plist-get (plist-get (plist-get lumenna--view :details) :task) :title)
                                 "Reminders")))))
        (with-current-buffer buffer
          (lumenna-reminder-list-mode)
          (lumenna--attach screen)
          (setq lumenna--view view lumenna--lines lines)
          (lumenna--redraw))
        (pop-to-buffer-same-window buffer)))))

(add-hook 'lumenna-followed-hook #'lumenna-reminders--open-list)

(defun lumenna-reminders--close ()
  "The reminder list is closed with its buffer."
  (when (and lumenna--screen lumenna--connection (jsonrpc-running-p lumenna--connection))
    (ignore-errors (lumenna-send "CloseReminders"))))

(defun lumenna-reminders-add ()
  "Add a reminder of this task's or block's own, by a phrase the core reads.
One the core cannot read is asked again, with the core's sentence first."
  (interactive)
  (lumenna-act (or (car (append (plist-get (lumenna-reminders--list) :actions) nil))
                   (user-error "Nothing adds a reminder here"))))

;;;; Menus

(lumenna-define-keys lumenna-reminders-due-mode
  ("The reminder at point"
   ("RET" "Open what it is about" lumenna-activate)
   ("d" "Dismiss" lumenna-reminders-dismiss)
   ("z" "Snooze" lumenna-reminders-snooze)
   ("c" "Carry Forward, where it offers to" lumenna-reminders-carry)
   ("r" "Replan from Now, where it offers to" lumenna-reminders-replan)))

(lumenna-define-keys lumenna-reminder-list-mode
  ("Reminders"
   ("a" "Add Reminder" lumenna-reminders-add)
   ("d" "Remove Reminder" lumenna-act-delete)))

(provide 'lumenna-reminders)

;;; lumenna-reminders.el ends here
