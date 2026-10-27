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
;; What came due and was not dealt with is the app's list, the same on every
;; device's apps, shown in M-x lumenna-reminders-due,
;; where d dismisses one, z snoozes it, and c carries forward what a block's
;; end left short, where its reminder offers to.  A task's or block's own
;; reminders are its Reminders action (R): a list of them, a to add one by a
;; phrase the core reads ("5m before start"), d to remove one.  When each
;; fires, what it says and the buttons' words are all the core's.

;;; Code:

(require 'lumenna)
(require 'parse-time)
;; Only where Emacs was built with D-Bus: elsewhere loading it signals.
(when (featurep 'dbusbind)
  (require 'notifications nil t))

(declare-function notifications-notify "notifications" (&rest params))
(declare-function lumenna-task-show "lumenna-tasks" (id))
(declare-function lumenna-today "lumenna-day" ())
(declare-function dbus-ping "dbus" (bus service &optional timeout))
(declare-function lumenna-replan "lumenna-proposal" (&optional event))

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

;;;; Giving them

;; `reminder.deliver' says what came due while this connection holds the
;; device's reminder lock; the app is then handed the same moments to look at
;; (`Due(Look)'), and what it finds it lists, says and gives as notices.  Which
;; are still listed, and what each button does, are the app's, as on every
;; other platform.

(defun lumenna-reminders--desktop-p ()
  "Whether a desktop notification can be shown here."
  (and lumenna-reminders-desktop (featurep 'notifications) (fboundp 'notifications-notify)
       (featurep 'dbusbind)
       (ignore-errors (dbus-ping :session "org.freedesktop.Notifications" 500))))

(defun lumenna-reminders--moment (time)
  "TIME as the RFC 3339 moment the core reads, in UTC."
  (format-time-string "%FT%TZ" time t))

(defun lumenna-reminders--hand-over (found now)
  "Have the app look at what FOUND, the core's `DueReminders' at NOW, gave.
It looks from just before the earliest of them until NOW, so it finds the
same reminders, and lists, says and shows each it does not list already."
  (let ((firings (append (plist-get found :missed) (plist-get found :due) nil)))
    (when firings
      (let ((since (lumenna-reminders--moment
                    (time-subtract (apply #'min (mapcar (lambda (f) (float-time (parse-iso8601-time-string (plist-get f :at))))
                                                        firings))
                                   1))))
        (lumenna-send (list :Due (list :Look (list :since since :now now))) 0)))))

(defun lumenna-reminders--notify (due)
  "Show DUE, a reminder the app lists, as a desktop notification.
Its buttons are the core's words, and each is the app's to carry out."
  (when (lumenna-reminders--desktop-p)
    (let* ((id (plist-get due :id))
           (first (aref (plist-get due :firings) 0)))
      (ignore-errors
        (notifications-notify
         :title (plist-get due :title) :body (plist-get due :text) :app-name "Lumenna"
         ;; The app's buttons, in the order every platform shows them: what the reminder
         ;; offers of its own (Carry Forward, Replan), then Dismiss and Snooze.
         :actions (append (mapcan (lambda (button) (list (plist-get button :id) (plist-get button :title)))
                                  (append (plist-get due :buttons) nil))
                          ;; What was missed, as one summary, is about nothing to open.
                          (and (plist-get due :snooze) (list "default" (plist-get first :title))))
         :on-action (lambda (_id action)
                      (pcase action
                        ("dismiss" (lumenna-reminders--deal id "Dismiss"))
                        ("snooze" (lumenna-reminders--deal id "Snooze"))
                        ("carry" (lumenna-reminders--deal id "Carry"))
                        ("replan" (lumenna-replan (list :Due (list :Replan id))))
                        ("default" (lumenna-reminders--open first)))))))))

(defun lumenna-reminders--noticed (turn)
  "Show each notice TURN gives as a desktop notification: a reminder come due."
  (let ((listed (append (plist-get (plist-get turn :view) :due) nil)))
    (dolist (notice (append (plist-get turn :notices) nil))
      (when-let* ((due (seq-find (lambda (d) (equal (plist-get d :id) (plist-get notice :id))) listed)))
        (lumenna-reminders--notify due)))))

(add-hook 'lumenna-turned-functions #'lumenna-reminders--noticed)

(defun lumenna-reminders-deliver ()
  "Ask what came due, and give it.  In the background; Emacs stays free."
  (interactive)
  (unless lumenna-reminders--asking
    (setq lumenna-reminders--asking t)
    (let ((now (if lumenna-reminders--now (funcall lumenna-reminders--now) (lumenna-reminders--moment nil))))
      (condition-case err
          (jsonrpc-async-request
           (lumenna--connection) 'reminder.deliver (list :now now)
           :success-fn (lambda (found)
                         (setq lumenna-reminders--asking nil)
                         (lumenna-reminders--pace (lumenna--true (plist-get found :elsewhere)))
                         (lumenna-reminders--hand-over found now))
           :error-fn (lambda (err)
                       (setq lumenna-reminders--asking nil)
                       (message "Lumenna reminders: %s" (plist-get err :message)))
           :timeout-fn (lambda () (setq lumenna-reminders--asking nil))
           :timeout 30)
        (error (setq lumenna-reminders--asking nil)
               (message "Lumenna reminders: %s" (error-message-string err)))))))

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

(defvar lumenna-reminders--resume nil
  "Whether `lumenna-disconnect' stopped the reminders.
They start again on connecting.")

(defun lumenna-reminders--connected ()
  "Start giving reminders on connecting, if that is wanted.
Or if a disconnection stopped them."
  (when (and (or lumenna-reminders-start-automatically lumenna-reminders--resume)
             (not lumenna-reminders-mode))
    (lumenna-reminders-mode 1))
  (setq lumenna-reminders--resume nil))

(defun lumenna-reminders--disconnected ()
  "Stop giving reminders once disconnected: asking would connect again at once."
  (when lumenna-reminders-mode
    (lumenna-reminders-mode -1)
    (setq lumenna-reminders--resume t)))

(add-hook 'lumenna-connected-hook #'lumenna-reminders--connected)
(add-hook 'lumenna-disconnected-hook #'lumenna-reminders--disconnected)

;;;; Dealing with one

(defun lumenna-reminders--deal (id event)
  "Send EVENT, a button of the app's (\"Dismiss\", \"Snooze\", \"Carry\"), for
the reminder due ID.  The app takes it off the list and says what it did."
  (with-current-buffer (lumenna--home-buffer)
    (setq lumenna--acting (pcase event
                            ("Dismiss" "reminder/dismiss") ("Snooze" "reminder/snooze")
                            ("Carry" "block/carry_rest")))
    (lumenna-send (list :Due (list (intern (concat ":" event)) id)) 0)
    (lumenna--follow)))

(defun lumenna-reminders--open (firing)
  "Open what FIRING, a reminder's, is about: its task, or its block's day."
  (pcase (plist-get firing :kind)
    ("task" (lumenna-task-show (plist-get firing :target)))
    (_ (with-current-buffer (lumenna-today)
         (lumenna-send (list :ShowBlock (list :series (plist-get firing :target) :date (plist-get firing :occurrence))))
         (lumenna--follow)))))

;;;; What came due

;; The app's list of what came due, drawn from the places' screen, whose
;; turns carry it.

(define-derived-mode lumenna-reminders-due-mode lumenna-list-mode "Lumenna Due"
  "What came due and was not dealt with yet.  d dismisses one, z snoozes it.

\\{lumenna-reminders-due-mode-map}"
  (setq-local lumenna--draw #'lumenna-reminders--due-rows)
  ;; The app's side list: after a button, the reminder that took its place is in hand.
  (setq-local lumenna--held :due_held)
  (setq-local lumenna--activate (lambda (row) (lumenna-reminders--open (aref (plist-get (plist-get row :due) :firings) 0)))))

(defun lumenna-reminders--due-rows ()
  "The reminders the app lists as due, as rows."
  (let ((due (append (plist-get lumenna--view :due) nil)))
    (setq-local lumenna--empty "Nothing has come due")
    (cons (if due (format "Reminders due, %d" (length due)) "Reminders due")
          (mapcar (lambda (one)
                    (list :key (plist-get one :id) :role "reminder" :due one :title (plist-get one :text)))
                  due))))

;;;###autoload
(defun lumenna-reminders-due ()
  "List what came due and was not dealt with: d dismisses one, z snoozes it."
  (interactive)
  (let ((home (lumenna--home-buffer))
        (buffer (get-buffer-create "*Lumenna: Reminders due*")))
    (with-current-buffer buffer
      (unless (derived-mode-p 'lumenna-reminders-due-mode)
        (lumenna-reminders-due-mode)
        (lumenna--attach 0))
      (setq lumenna--view (buffer-local-value 'lumenna--view home)
            lumenna--lines (buffer-local-value 'lumenna--lines home))
      (lumenna--redraw))
    (pop-to-buffer-same-window buffer)))

(defun lumenna-reminders--at-point ()
  "The reminder due at point, as the app lists it, or a `user-error'."
  (or (plist-get (lumenna-row) :due) (user-error "No reminder on this line")))

(defun lumenna-reminders--press (event button)
  "Send EVENT for the reminder at point.
It offers it if it has BUTTON's words; one it does not offer is refused in
the app's words."
  (let* ((due (lumenna-reminders--at-point))
         (id (plist-get due :id)))
    (if (plist-get due button)
        (lumenna-reminders--deal id event)
      (let ((lumenna--heard (list 'heard)))
        (lumenna-send (list :Due (list (intern (concat ":" event)) id)) 0)
        (lumenna--settle)
        (user-error "%s" (string-join (or (cdr lumenna--heard) '("Nothing to do here")) " "))))))

(defun lumenna-reminders-dismiss ()
  "Dismiss the reminder at point, on every device."
  (interactive)
  (lumenna-reminders--press "Dismiss" :dismiss))

(defun lumenna-reminders-snooze ()
  "Snooze the reminder at point, on every device, for as long as the core says."
  (interactive)
  ;; What was missed, as one summary, has no Snooze: the app refuses it in its words.
  (lumenna-reminders--press "Snooze" :snooze))

(defun lumenna-reminders-carry ()
  "Carry what the block of the reminder at point left short forward, as it offers."
  (interactive)
  (lumenna-reminders--press "Carry" :carry))

(defun lumenna-reminders-replan ()
  "Open Replan from Now, as the reminder at point offers to.
Once something is replanned, the app dismisses the warning."
  (interactive)
  (let ((due (lumenna-reminders--at-point)))
    (if (plist-get due :replan)
        (lumenna-replan (list :Due (list :Replan (plist-get due :id))))
      (lumenna-reminders--press "Replan" :replan))))

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
                               :text (plist-get line :text) :actions (plist-get reminder :offered)))
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
  (lumenna-act (or (car (append (plist-get (lumenna-reminders--list) :offered) nil))
                   (user-error "Nothing adds a reminder here"))
               nil "Reminders"))

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
