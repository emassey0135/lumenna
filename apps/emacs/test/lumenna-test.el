;;; lumenna-test.el --- Lumenna's Emacs client against a real lum rpc -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Elijah Massey
;; SPDX-License-Identifier: MPL-2.0

;;; Commentary:

;; Each test drives the client as a person would -- opens a list, moves to a
;; line, presses a command's key -- against a real `lum rpc' on a scratch
;; profile, then asks the store whether it happened.  What a person types in
;; the minibuffer is supplied by binding the reading functions.
;;
;;   emacs --batch -Q -L apps/emacs -l apps/emacs/test/lumenna-test.el \
;;     -f ert-run-tests-batch-and-exit
;;
;; LUM names the binary; otherwise target/debug/lum in this checkout is used.

;;; Code:

(require 'ert)
(require 'lumenna)

;; Answering the minibuffer means replacing built-in functions for a moment, for
;; which native compilation would build a trampoline each; not needed here, and
;; some builds' native compilers are broken.
(setq native-comp-enable-subr-trampolines nil)

(defvar lumenna-test--root
  (expand-file-name "../../.." (file-name-directory (or load-file-name buffer-file-name))))

(defun lumenna-test--lum ()
  (or (getenv "LUM") (expand-file-name "target/debug/lum" lumenna-test--root)))

(defmacro lumenna-test--with-store (&rest body)
  "Run BODY against a fresh store, with its own `lum rpc'."
  (declare (indent 0))
  `(let* ((profile (make-temp-file "lumenna-test" t))
          (lumenna-profile profile)
          (lumenna-lum-program (lumenna-test--lum))
          (process-environment (cons (concat "LUMENNA_BACKUP_DIR=" profile "/backups") process-environment))
          (lumenna--connection nil)
          (lumenna-reminders-start-automatically nil)
          (lumenna-announce-function #'ignore))
     (skip-unless (file-executable-p lumenna-lum-program))
     (unwind-protect
         (progn ,@body)
       (lumenna-disconnect)
       (dolist (buffer (buffer-list))
         (when (string-prefix-p "*Lumenna" (buffer-name buffer)) (kill-buffer buffer)))
       (delete-directory profile t))))

(defmacro lumenna-test--answering (answers &rest body)
  "Run BODY with each minibuffer question answered from ANSWERS, in order."
  (declare (indent 1))
  `(let ((queue (list ,@answers)))
     (cl-letf (((symbol-function 'read-string) (lambda (&rest _) (pop queue)))
               ((symbol-function 'read-from-minibuffer) (lambda (&rest _) (pop queue)))
               ((symbol-function 'completing-read) (lambda (&rest _) (pop queue)))
               ((symbol-function 'completing-read-multiple) (lambda (&rest _) (pop queue)))
               ((symbol-function 'read-number) (lambda (&rest _) (pop queue)))
               ((symbol-function 'yes-or-no-p) (lambda (&rest _) (pop queue)))
               ((symbol-function 'y-or-n-p) (lambda (&rest _) (pop queue))))
       ,@body
       (should (null queue)))))

(defun lumenna-test--seed (method &rest params)
  "Change the store with METHOD and PARAMS, as another client would, and wait
until the app has taken it in."
  (prog1 (apply #'lumenna-call method params)
    (lumenna--tick)
    (lumenna--settle)))

(defun lumenna-test--goto (text)
  "Point on the line containing TEXT."
  (goto-char (point-min))
  (should (search-forward text nil t))
  (beginning-of-line))

(defun lumenna-test--titles (&optional query)
  (mapcar (lambda (row) (plist-get row :title))
          (append (plist-get (if query (lumenna-call "task.list" :query query) (lumenna-call "task.list")) :rows) nil)))

(defun lumenna-test--act-named (title)
  "Do the action called TITLE on the row at point, as `.' would choose it."
  (lumenna-act (or (seq-find (lambda (a) (equal (plist-get a :title) title)) (lumenna-actions))
                   (ert-fail (format "no %s here" title)))
               (lumenna--row-here)))

(defun lumenna-test--line ()
  (buffer-substring-no-properties (line-beginning-position) (line-end-position)))

;;;; Reminders

(defun lumenna-test--deliver ()
  "Ask what came due, as the timer does, and wait for the answer and the app's look."
  (lumenna-reminders-deliver)
  (while lumenna-reminders--asking (accept-process-output nil 0.05))
  (lumenna--settle))

(defun lumenna-test--due ()
  "What the app lists as due."
  (append (plist-get (buffer-local-value 'lumenna--view (lumenna--home-buffer)) :due) nil))

(defmacro lumenna-test--reminding (&rest body)
  "Run BODY against a fresh store in UTC, delivering at `now', a variable BODY sets.
`said' collects what was announced and `shown' each desktop notification."
  (declare (indent 0))
  `(let ((process-environment (cons "TZ=UTC" process-environment)))
     (lumenna-test--with-store
       (let* ((now "2030-01-15T08:50:00Z")
              (lumenna-reminders--now (lambda () now))
              said shown)
         (cl-letf (((symbol-function 'lumenna-reminders--desktop-p) (lambda () t))
                   ((symbol-function 'notifications-notify) (lambda (&rest params) (push params shown) 1)))
           (let ((lumenna-announce-function (lambda (announcement _notices) (push announcement said))))
             ,@body))))))

(ert-deftest lumenna-a-reminder-is-said-once-shown-with-the-cores-buttons-and-dismissed ()
  (lumenna-test--reminding
    (lumenna-test--seed "block.add" :title "Deep work" :at "9:00" :minutes 60 :date "2030-01-15")
    (setq said nil)
    (lumenna-test--deliver)
    (should-not said)
    (setq now "2030-01-15T08:55:30Z")
    (lumenna-test--deliver)
    (should (equal said '("Deep work starts in 5 minutes — nothing assigned yet")))
    (let ((notification (car shown)))
      (should (= (length shown) 1))
      (should (equal (plist-get notification :title) "Deep work"))
      (should (equal (plist-get notification :actions)
                     '("dismiss" "Dismiss" "snooze" "Snooze 10 Minutes" "default" "Deep work")))
      (setq now "2030-01-15T08:56:00Z")
      (lumenna-test--deliver)
      (should (= (length said) 1))
      (funcall (plist-get notification :on-action) 1 "dismiss"))
    (should (equal (car said) "Dismissed"))
    (should-not (lumenna-test--due))
    (should-not (append (plist-get (lumenna-call "reminder.due" :since "2030-01-15T08:50:00Z"
                                                 :now "2030-01-15T08:59:00Z")
                                   :due)
                        nil))))

(ert-deftest lumenna-while-another-process-gives-reminders-emacs-asks-every-five-seconds-silently ()
  (lumenna-test--reminding
    (let (other
          (lumenna-reminders-mode t)
          (lumenna-reminders--every lumenna-reminders-interval)
          (lumenna-reminders--timer nil))
      (unwind-protect
          (progn
            ;; Another connection takes the right first.
            (let ((lumenna--connection nil))
              (lumenna-call "reminder.deliver" :now "2030-01-15T08:40:00Z")
              (setq other lumenna--connection))
            (setq said nil)
            (lumenna-test--deliver)
            (should (= lumenna-reminders--every 5))
            (should-not said)
            (jsonrpc-shutdown other)
            (sleep-for 0.3)
            (lumenna-test--deliver)
            (should (= lumenna-reminders--every 20)))
        (when lumenna-reminders--timer (cancel-timer lumenna-reminders--timer))
        (when other (ignore-errors (jsonrpc-shutdown other)))))))

(ert-deftest lumenna-what-came-while-nothing-ran-is-one-summary-dismissed-from-the-due-list ()
  (lumenna-test--reminding
    (lumenna-test--seed "block.add" :title "Deep work" :at "9:00" :minutes 60 :date "2030-01-15")
    (lumenna-test--deliver)
    (setq now "2030-01-15T09:30:00Z" said nil)
    (lumenna-test--deliver)
    (should (equal said '("1 reminder missed: Deep work started.")))
    (should (equal (plist-get (car shown) :actions) '("dismiss" "Dismiss")))
    (lumenna-reminders-due)
    (lumenna-test--goto "1 reminder missed")
    (lumenna-reminders-dismiss)
    (goto-char (point-min))
    (should (equal (lumenna-test--line) "Reminders due"))
    (forward-line 1)
    (should (equal (lumenna-test--line) "Nothing has come due"))))

(ert-deftest lumenna-a-summary-of-what-was-missed-is-not-snoozed-in-the-apps-words ()
  (lumenna-test--reminding
    (lumenna-test--seed "block.add" :title "Deep work" :at "9:00" :minutes 60 :date "2030-01-15")
    (lumenna-test--deliver)
    (setq now "2030-01-15T09:30:00Z")
    (lumenna-test--deliver)
    (lumenna-reminders-due)
    (lumenna-test--goto "1 reminder missed")
    (should (equal (cadr (should-error (lumenna-reminders-snooze) :type 'user-error))
                   "What was missed can be dismissed, not snoozed."))
    (should (= (length (lumenna-test--due)) 1))))

(ert-deftest lumenna-a-reminder-dismissed-elsewhere-leaves-the-due-list ()
  (lumenna-test--reminding
    (lumenna-test--seed "block.add" :title "Deep work" :at "9:00" :minutes 60 :date "2030-01-15")
    (lumenna-test--deliver)
    (setq now "2030-01-15T08:55:30Z")
    (lumenna-test--deliver)
    (lumenna-reminders-due)
    (lumenna-test--goto "Deep work starts in 5 minutes")
    (let ((firing (aref (plist-get (car (lumenna-test--due)) :firings) 0)))
      ;; Dismissed as another device would, through the store: the app learns it from the store.
      (lumenna-test--seed "reminder.dismiss" :key (plist-get firing :key) :occurrence (plist-get firing :occurrence)))
    (should-not (lumenna-test--due))
    (goto-char (point-min))
    (forward-line 1)
    (should (equal (lumenna-test--line) "Nothing has come due"))))

(ert-deftest lumenna-a-blocks-reminders-are-listed-added-by-a-phrase-and-removed ()
  (lumenna-test--with-store
    (lumenna-test--seed "block.add" :title "Deep work" :at "9:00" :minutes 60 :date "tomorrow")
    (lumenna-blocks)
    (lumenna-test--goto "Deep work")
    (lumenna-act-reminders)
    (should (equal (buffer-name) "*Lumenna reminders: Deep work*"))
    (goto-char (point-min))
    (should (equal (lumenna-test--line) "1 reminder, the defaults"))
    (forward-line 1)
    (should (equal (lumenna-test--line) "5 minutes before it starts, a default"))
    (let ((answers '("when it feels right" "10m after end")) prompts)
      (cl-letf (((symbol-function 'read-string) (lambda (prompt &rest _) (push prompt prompts) (pop answers))))
        (lumenna-reminders-add))
      (should (= (length prompts) 2))
      (should (string-prefix-p "'when it feels right' is not a reminder" (car prompts))))
    (goto-char (point-min))
    (should (equal (lumenna-test--line) "1 reminder"))
    (lumenna-test--goto "10 minutes after it ends")
    (lumenna-act-delete)
    (goto-char (point-min))
    (should (equal (lumenna-test--line) "1 reminder, the defaults"))))

(ert-deftest lumenna-a-tasks-priority-is-its-face-which-a-voice-reads ()
  (lumenna-test--with-store
    (lumenna-test--seed "task.add" :text "file taxes p1")
    (lumenna-test--seed "task.add" :text "water plants p3")
    (lumenna-test--seed "task.add" :text "someday")
    (lumenna-tasks)
    (dolist (case '(("file taxes" . lumenna-priority-1) ("water plants" . lumenna-priority-3) ("someday" . nil)))
      (lumenna-test--goto (car case))
      (should (eq (get-text-property (line-beginning-position) 'face) (cdr case))))
    (require 'lumenna-voice)
    (should (assq 'lumenna-priority-1 lumenna-voice-map))))

(ert-deftest lumenna-a-task-with-no-due-date-offers-reminders-at-a-moment ()
  (lumenna-test--with-store
    (lumenna-test--seed "task.add" :text "someday")
    (lumenna-tasks)
    (lumenna-test--goto "someday")
    (let* ((heard nil)
           (lumenna-announce-function (lambda (text _) (push text heard))))
      (lumenna-act-reminders)
      (should (equal (buffer-name) "*Lumenna reminders: someday*"))
      (should (seq-some (lambda (said) (string-match-p "a moment of its own" said)) heard)))))

(ert-deftest lumenna-a-default-reminder-the-core-refuses-is-asked-again-with-its-sentence ()
  (lumenna-test--with-store
    (lumenna-settings)
    (lumenna-test--goto "Reminders for a block")
    (let ((answers '("at the end of time" "at end")) prompts)
      (cl-letf (((symbol-function 'read-string) (lambda (prompt initial &rest _) (push (cons prompt initial) prompts) (pop answers))))
        (lumenna-activate))
      (should (= (length prompts) 2))
      (should (string-match-p "not a reminder\\|not 'the" (car (car prompts))))
      (should (equal (cdr (car prompts)) "at the end of time")))
    (should (equal (plist-get (aref (plist-get (lumenna-call "config.get" :key "default-block-reminders") :settings) 0) :value)
                   "at end"))))

;;;; Tasks

(ert-deftest lumenna-a-weight-is-read-by-the-core-which-refuses-a-typo ()
  (lumenna-test--with-store
    (lumenna-test--seed "project.add" :name "Work")
    (lumenna-projects)
    (lumenna-test--goto "Work")
    ;; Refused, the question is asked again with why, and what was typed.
    (let (prompts (answers (list "1,5" "1.5")))
      (cl-letf (((symbol-function 'read-string) (lambda (prompt &rest _) (push prompt prompts) (pop answers))))
        (lumenna-act-weight))
      (should (= (length prompts) 2))
      (should (string-match-p "1,5" (car prompts))))
    (lumenna-test--goto "Work")
    (should (string-match-p "1.5" (lumenna-test--line)))))

(ert-deftest lumenna-a-key-runs-only-what-the-row-offers ()
  (lumenna-test--with-store
    (lumenna-projects)
    (lumenna-test--goto "Inbox")
    ;; The Inbox keeps its name: the core offers no Rename, so the key refuses.
    (let ((refusal (should-error (lumenna-act-rename) :type 'user-error)))
      (should (equal (cadr refusal)
                     "The Inbox keeps its name and its place; only its order and weight change.")))
    (let (offered)
      (cl-letf (((symbol-function 'completing-read)
                 (lambda (_prompt choices &rest _) (setq offered (mapcar #'car choices)) (car offered)))
                ((symbol-function 'read-string) (lambda (&rest _) "1")))
        (lumenna-act-at-point))
      (should (equal offered '("Weight"))))))

(ert-deftest lumenna-a-project-with-a-space-is-quoted-by-the-core-in-its-list ()
  (lumenna-test--with-store
    (lumenna-test--seed "project.add" :name "Home Office")
    (lumenna-test--seed "task.add" :text "file the receipts #\"Home Office\"")
    (lumenna-projects)
    (lumenna-test--goto "Home Office")
    (lumenna-activate)
    (should (equal (plist-get lumenna--view :quick_add) "#\"Home Office\" "))
    (should (equal (lumenna-test--titles "#\"Home Office\"") '("file the receipts")))
    (goto-char (point-min))
    (should (search-forward "file the receipts" nil t))))

(ert-deftest lumenna-a-pick-with-nothing-to-offer-says-the-cores-sentence ()
  (lumenna-test--with-store
    (lumenna-test--seed "task.add" :text "only one")
    (lumenna-tasks)
    (lumenna-test--goto "only one")
    (let (said)
      (let ((lumenna-announce-function (lambda (text _) (setq said text))))
        (lumenna-test--answering ()
          (lumenna-act-wait-for)))
      (should (equal said "There is no task it could wait for.")))))

(ert-deftest lumenna-a-task-due-at-a-time-says-it-in-emacss-clock-before-its-priority ()
  (lumenna-test--with-store
    (lumenna-test--seed "task.add" :text "call the bank tomorrow at 3pm p1")
    (let ((display-time-24hr-format t))
      (lumenna-tasks)
      (lumenna-test--goto "call the bank")
      (should (equal (lumenna-test--line) "call the bank, due tomorrow at 15:00, priority 1")))))

(ert-deftest lumenna-a-task-is-listed-in-words-completed-and-brought-back-with-undo ()
  (lumenna-test--with-store
    (lumenna-test--seed "task.add" :text "buy milk tomorrow")
    (lumenna-tasks)
    (lumenna-test--goto "buy milk")
    (should (string-match-p "\\`buy milk, due " (lumenna-test--line)))
    (lumenna-act-done)
    (should (equal (lumenna-test--titles) nil))
    (lumenna-undo)
    (should (equal (lumenna-test--titles) '("buy milk")))
    (should (string-prefix-p "buy milk" (lumenna-test--line)))))

(ert-deftest lumenna-quick-add-refuses-an-unknown-project-and-adds-otherwise ()
  (lumenna-test--with-store
    (lumenna-test--answering ("water the plants #Nowhere")
      (should-error (lumenna-add) :type 'user-error))
    (should (equal (lumenna-test--titles) nil))
    (lumenna-test--answering ("water the plants every monday p2")
      (lumenna-add))
    (should (equal (lumenna-test--titles) '("water the plants")))))

(ert-deftest lumenna-a-subtask-sits-under-its-task-a-level-deeper ()
  (lumenna-test--with-store
    (lumenna-test--seed "task.add" :text "paint the room")
    (lumenna-test--seed "task.add" :text "buy paint")
    (lumenna-tasks)
    (lumenna-test--goto "buy paint")
    (lumenna-test--answering ("paint the room, Inbox")
      (lumenna-act-make-subtask))
    (lumenna-test--goto "  buy paint")
    (should (equal (funcall outline-level) 3))
    (lumenna-test--goto "paint the room")
    (should (equal (funcall outline-level) 2))))

(ert-deftest lumenna-the-detail-buffer-changes-a-field-and-keeps-a-repetition ()
  (lumenna-test--with-store
    (lumenna-test--seed "task.add" :text "water plants every monday")
    (lumenna-tasks)
    (lumenna-test--goto "water plants")
    (lumenna-activate)
    (should (derived-mode-p 'lumenna-task-mode))
    (lumenna-test--goto "Repeats: every monday")
    (lumenna-test--goto "Due:")
    (lumenna-test--answering ("2026-12-10")
      (lumenna-activate))
    (lumenna-test--goto "Due: 2026-12-10")
    (lumenna-test--goto "Repeats: every monday")
    (lumenna-test--goto "Labels:")
    (lumenna-test--answering ('("garden" "calls"))
      (lumenna-task-edit))
    (lumenna-test--goto "Labels: garden, calls")))

(ert-deftest lumenna-the-trash-restores-and-erases-asking-first ()
  (lumenna-test--with-store
    (lumenna-test--seed "task.add" :text "keep me")
    (lumenna-test--seed "task.add" :text "lose me")
    (lumenna-tasks)
    (lumenna-test--goto "keep me")
    (lumenna-act-delete)
    (lumenna-test--goto "lose me")
    (lumenna-act-delete)
    (lumenna-trash)
    (lumenna-test--goto "keep me")
    (lumenna-activate)
    (lumenna-test--goto "lose me")
    (lumenna-test--answering (t)
      (lumenna-act-delete))
    (should (equal (lumenna-test--titles) '("keep me")))
    (should (equal (lumenna-test--titles "deleted") nil))))

(ert-deftest lumenna-a-trashed-task-is-erased-permanently-only-by-name-and-its-delete-key-deletes-from-trash ()
  (lumenna-test--with-store
    (lumenna-test--seed "task.add" :text "kept in the trash")
    (lumenna-test--seed "task.add" :text "the secret")
    (lumenna-tasks)
    (lumenna-test--goto "kept in the trash")
    (lumenna-act-delete)
    (lumenna-test--goto "the secret")
    (lumenna-act-delete)
    (lumenna-trash)
    ;; d asks Delete from Trash's question, which undo can take back; turned down, nothing goes.
    (lumenna-test--goto "kept in the trash")
    (let (asked)
      (cl-letf (((symbol-function 'yes-or-no-p) (lambda (prompt) (setq asked prompt) nil)))
        (lumenna-act-delete))
      (should (string-prefix-p "Undo can bring it back" asked)))
    (should (equal (lumenna-test--titles "deleted") '("kept in the trash" "the secret")))
    ;; Erase Permanently is offered last, by name only, and says what it does first.
    (lumenna-test--goto "the secret")
    (let (offered asked said changed)
      (cl-letf (((symbol-function 'completing-read)
                 (lambda (_prompt choices &rest _) (setq offered (mapcar #'car choices)) "Erase Permanently"))
                ((symbol-function 'yes-or-no-p) (lambda (prompt) (setq asked prompt) t)))
        (let ((lumenna-announce-function (lambda (announcement _notices) (push announcement said)))
              (lumenna-changed-functions (list (lambda (event _) (push event changed)))))
          (lumenna-act-at-point)))
      (should (equal offered '("Restore" "Delete from Trash" "Erase Permanently")))
      (should (string-prefix-p "Permanently erase — rebuilds your database and re-syncs all devices" asked))
      (should (string-suffix-p "Erase the secret permanently? " asked))
      (should (member "Erased the secret permanently" said))
      (should (equal changed '("task/erase_permanently"))))
    (should (equal (lumenna-test--titles "deleted") '("kept in the trash")))
    ;; d is the core's delete key: here, Delete from Trash, its sound keyed on what the core ran.
    (lumenna-test--goto "kept in the trash")
    (let (changed)
      (cl-letf (((symbol-function 'yes-or-no-p) (lambda (_) t)))
        (let ((lumenna-changed-functions (list (lambda (event _) (push event changed)))))
          (lumenna-act-delete)))
      (should (equal changed '("task/delete_for_good"))))
    (should (equal (lumenna-test--titles "deleted") nil))))

(ert-deftest lumenna-settings-offer-erasing-everything-deleted-as-a-button-which-asks-first ()
  (lumenna-test--with-store
    (lumenna-test--seed "task.add" :text "the secret")
    (lumenna-tasks)
    (lumenna-test--goto "the secret")
    (lumenna-act-delete)
    (lumenna-settings)
    ;; A line of its own after the settings, named by the core's title.
    (lumenna-test--goto "Erase Everything Deleted")
    (should (equal (lumenna-test--line) "Erase Everything Deleted"))
    ;; The core's caption follows it, quiet but read.
    (forward-line 1)
    (should (equal (lumenna-test--line)
                   "Takes what was deleted out of the history every device keeps, so nothing can bring it back."))
    (should (eq (get-text-property (point) 'face) 'lumenna-quiet))
    (should-error (lumenna-activate) :type 'user-error)
    (forward-line -1)
    (should (equal (mapcar (lambda (a) (plist-get a :title)) (lumenna-actions)) '("Erase Everything Deleted")))
    ;; Turned down, nothing is erased.
    (cl-letf (((symbol-function 'yes-or-no-p) (lambda (_) nil)))
      (lumenna-activate))
    (should (equal (lumenna-test--titles "deleted") '("the secret")))
    (let (asked said changed)
      (cl-letf (((symbol-function 'yes-or-no-p) (lambda (prompt) (setq asked prompt) t)))
        (let ((lumenna-announce-function (lambda (announcement _notices) (push announcement said)))
              (lumenna-changed-functions (list (lambda (event _) (push event changed)))))
          (lumenna-activate)))
      (should (string-prefix-p "Permanently erase — rebuilds your database and re-syncs all devices" asked))
      (should (string-suffix-p "Erase everything deleted permanently? " asked))
      (should (member "Erased 1 task and the history of everything deleted" said))
      (should (equal changed '("store/erase_everything_deleted"))))
    (should (equal (lumenna-test--titles "deleted") nil))
    ;; Settings is still there to use, on the store begun again.
    (should (derived-mode-p 'lumenna-settings-mode))
    (lumenna-test--goto "Erase Everything Deleted")))

;;;; The day

(ert-deftest lumenna-the-day-opens-on-now-and-plans-a-sitting ()
  (lumenna-test--with-store
    (lumenna-test--seed "task.add" :text "write the chapter")
    (lumenna-test--seed "block.add" :title "Focus" :at "00:00" :minutes 1440 :date "today")
    (lumenna-today)
    (should (string-match-p "Focus" (lumenna-test--line)))
    (lumenna-test--answering ("write the chapter, Inbox" "45m")
      (lumenna-act-assign))
    (lumenna-test--goto "write the chapter")
    (should (string-match-p "planned for 45 minutes" (lumenna-test--line)))
    (lumenna-act-timer)
    (lumenna-test--goto "write the chapter")
    (should (string-match-p "in progress, 45 minutes planned" (lumenna-test--line)))
    (lumenna-test--answering ("")
      (lumenna-act-planned-length))
    (lumenna-test--goto "write the chapter")
    (should-not (string-match-p "planned" (lumenna-test--line)))))

(ert-deftest lumenna-a-task-goes-in-a-work-block-the-core-offers-from-the-task ()
  (lumenna-test--with-store
    (lumenna-test--seed "task.add" :text "write the chapter")
    (lumenna-test--seed "block.add" :title "Focus" :at "9am" :minutes 90 :date "tomorrow")
    (lumenna-test--seed "block.add" :title "Lunch" :at "noon" :minutes 30 :kind "break" :date "tomorrow")
    (lumenna-tasks)
    (lumenna-test--goto "write the chapter")
    (let (offered)
      (cl-letf (((symbol-function 'completing-read)
                 (lambda (_prompt choices &rest _)
                   (setq offered (mapcar #'car choices))
                   (car offered)))
                ((symbol-function 'read-from-minibuffer) (lambda (&rest _) "30")))
        (lumenna-act-put-in-block))
      ;; Focus alone takes tasks; Another Day asks for a block further off.
      (should (equal (length offered) 2))
      (should (string-match-p ", 9:00 AM to 10:30 AM, Focus\\'" (car offered)))
      (should (equal (cadr offered) "Another Day")))
    (let ((plan (lumenna-call "plan" :date "tomorrow")))
      (should (equal (plist-get (aref (plist-get (aref (plist-get plan :blocks) 0) :assignments) 0) :planned_mins)
                     30)))))

(ert-deftest lumenna-a-sitting-pauses-resumes-and-stops ()
  (lumenna-test--with-store
    (lumenna-test--seed "task.add" :text "write the chapter")
    (lumenna-test--seed "block.add" :title "Focus" :at "00:00" :minutes 1440 :date "today")
    (lumenna-today)
    (lumenna-test--answering ("write the chapter, Inbox" "")
      (lumenna-act-assign))
    (lumenna-test--goto "write the chapter")
    (lumenna-act-timer)
    (lumenna-test--goto "write the chapter")
    (should (string-match-p "in progress" (lumenna-test--line)))
    (lumenna-act-timer)
    (lumenna-test--goto "write the chapter")
    (should (string-match-p "paused" (lumenna-test--line)))
    ;; Paused, the sitting is still in progress: Stop is offered, Start is not.
    (should-not (seq-find (lambda (a) (equal (plist-get a :kind) "start_timer")) (lumenna-actions)))
    (lumenna-act-timer)
    (lumenna-test--goto "write the chapter")
    (should (string-match-p "in progress" (lumenna-test--line)))
    (lumenna-act-stop-timer)
    (lumenna-test--goto "write the chapter")
    (should (string-match-p "worked" (lumenna-test--line)))))

(ert-deftest lumenna-a-block-is-changed-a-field-at-a-time-and-a-break-can-take-tasks ()
  (lumenna-test--with-store
    (lumenna-test--seed "block.add" :title "Train" :at "00:00" :minutes 1440 :date "today" :kind "break")
    (lumenna-today)
    (lumenna-test--goto "Train")
    (should-not (string-match-p "assigned" (lumenna-test--line)))
    (lumenna-test--answering ("Takes tasks" t)
      (lumenna-act-edit))
    (lumenna-test--goto "Train")
    (should (string-match-p "break block, now, takes tasks, nothing assigned" (lumenna-test--line)))
    (lumenna-test--answering ("Notes" "window seat")
      (lumenna-act-edit))
    (let ((id (plist-get (aref (plist-get (lumenna-call "block.list") :rows) 0) :id)))
      (should (equal (plist-get (lumenna-call "block.show" :id id) :notes) "window seat")))))

(ert-deftest lumenna-a-cancelled-day-is-listed-and-put-back ()
  (lumenna-test--with-store
    (lumenna-test--seed "block.add" :title "Run" :at "7am" :minutes 30 :date "today" :repeat "every day")
    (lumenna-today)
    (lumenna-test--goto "Run")
    (lumenna-act-cancel-day)
    (lumenna-test--goto "Run")
    (should (string-match-p "cancelled for this day" (lumenna-test--line)))
    (lumenna-activate)
    (lumenna-test--goto "Run")
    (should-not (string-match-p "cancelled" (lumenna-test--line)))))

(ert-deftest lumenna-the-day-moves-between-days ()
  (lumenna-test--with-store
    (lumenna-test--seed "block.add" :title "Dentist" :at "2pm" :minutes 60 :kind "event" :date "tomorrow")
    (lumenna-today)
    (should-not (lumenna--find "Dentist"))
    (lumenna-day-next)
    (goto-char (point-min))
    (should (search-forward "Dentist" nil t))
    (lumenna-day-previous)
    (goto-char (point-min))
    (should-not (search-forward "Dentist" nil t))))

;;;; The core's words

(ert-deftest lumenna-free-time-now-and-a-cancelled-day-are-said-in-the-cores-order ()
  (lumenna-test--with-store
    ;; From tomorrow: today, whose "now" is checked below, holds no block that now could fall
    ;; inside, whenever the test runs (at 07:08 UTC it fell inside today's Run, and the day said
    ;; the block under way instead).
    (lumenna-test--seed "block.add" :title "Run" :at "7am" :minutes 30 :date "tomorrow" :repeat "every day")
    (let ((display-time-24hr-format t))
      (lumenna-today)
      (should (seq-find (lambda (l) (string-match-p "\\`Now, [0-9][0-9]:[0-9][0-9]\\'" l)) (lumenna-test--lines)))
      (lumenna-day-next)
      (lumenna-test--goto "Run")
      (lumenna-act-cancel-day)
      (should (member "07:00, Run, cancelled for this day" (lumenna-test--lines)))
      (should (seq-find (lambda (l) (string-match-p "\\`Free, [0-9]+ hours?\\( [0-9]+ minutes?\\)?, [0-9:]+ to [0-9:]+\\'" l))
                        (lumenna-test--lines))))))

(ert-deftest lumenna-an-empty-list-says-the-cores-words-under-a-heading-without-its-count ()
  (lumenna-test--with-store
    (lumenna-blocks)
    (goto-char (point-min))
    (should (equal (lumenna-test--line) "Blocks"))
    (forward-line 1)
    (should (equal (lumenna-test--line) "No blocks."))
    (should-error (lumenna-row) :type 'user-error)
    ;; The saved filters are a heading of the places, as every app's sidebar has them.
    (lumenna-filters)
    (should (equal (buffer-string) "Saved Filters\n"))
    (lumenna-devices)
    (should (equal (buffer-string) "Devices and sync\nNo devices are paired yet. Pair one to sync with it.\n"))
    (lumenna-test--seed "label.add" :name "calls")
    (lumenna-labels)
    (should (equal (lumenna-test--lines) '("Labels" "  calls, no open tasks")))))

(ert-deftest lumenna-going-to-a-day-and-a-new-filter-ask-the-cores-questions ()
  (lumenna-test--with-store
    (lumenna-test--seed "block.add" :title "Dentist" :at "2pm" :minutes 60 :kind "event" :date "tomorrow")
    (lumenna-today)
    (let (asked (answers (list "tomorrow" "Urgent" "p1")))
      (cl-letf (((symbol-function 'read-from-minibuffer) (lambda (prompt &rest _) (push prompt asked) (pop answers)))
                ((symbol-function 'read-string) (lambda (prompt &rest _) (push prompt asked) (pop answers))))
        (call-interactively #'lumenna-day-go-to)
        (goto-char (point-min))
        (should (string-match-p "\\`Tomorrow\\. 1 block" (lumenna-test--line)))
        (should (search-forward "Dentist" nil t))
        (lumenna-filters)
        (lumenna-filter-add))
      (should (equal (reverse asked)
                     '("Go to day. A date, such as Friday, or 12 October. Day: "
                       "New saved filter. Name: "
                       "New saved filter. A filter, such as p1 & due before: friday. Query: "))))
    (should (equal (plist-get (aref (plist-get (lumenna-call "filter.list") :filters) 0) :name) "Urgent"))))

(ert-deftest lumenna-the-block-form-asks-in-the-cores-words-the-name-last ()
  (lumenna-test--with-store
    (let (asked examples (answers (list "Run" "today" "7am" "30" "Work" "")))
      (cl-letf (((symbol-function 'read-from-minibuffer)
                 (lambda (prompt _initial &optional _map _read _history example)
                   (push prompt asked) (push example examples) (pop answers)))
                ((symbol-function 'completing-read) (lambda (prompt &rest _) (push prompt asked) (pop answers))))
        (lumenna-add-block))
      ;; Each example is the field's, a M-n away; the kind is a choice.
      (should (equal (reverse examples) '("Deep work" "today" "9am" "60" "every weekday")))
      (should (equal (reverse asked)
                     `("Name: " "The day it happens, or the first day it repeats. Day: "
                       "A time, such as 9am or 14:30. Starts at: " "Lasts, in minutes: "
                       ,(format-message "Changing it sets the three choices after it to the kind's own. Kind (default Work): ")
                       "Such as every weekday. Empty for once. Repeats: "))))
    (should (equal (plist-get (aref (plist-get (lumenna-call "block.list") :rows) 0) :title) "Run"))))

(ert-deftest lumenna-a-new-block-starts-when-the-core-says-for-the-day-typed ()
  (lumenna-test--with-store
    (lumenna-test--seed "config.set" :key "day-start" :value "07:30")
    (let (initials (answers (list "Run" "tomorrow" nil "30" "Work" "")))
      (cl-letf (((symbol-function 'read-from-minibuffer)
                 (lambda (_prompt initial &rest _)
                   (push initial initials)
                   (let ((answer (pop answers))) (or answer initial))))
                ((symbol-function 'completing-read) (lambda (&rest _) (pop answers))))
        (lumenna-add-block))
      ;; Another day starts when the day does; the start field begins there.
      (should (equal (nth 2 (reverse initials)) "07:30")))
    (should (equal (plist-get (aref (plist-get (lumenna-call "block.list") :rows) 0) :due_time) "07:30"))))

(ert-deftest lumenna-a-destructive-question-says-what-it-does-then-asks-it ()
  (lumenna-test--with-store
    (lumenna-test--seed "label.add" :name "calls")
    (lumenna-labels)
    (lumenna-test--goto "calls")
    (let (asked)
      (cl-letf (((symbol-function 'yes-or-no-p) (lambda (prompt) (setq asked prompt) nil)))
        (lumenna-act-delete))
      (should (equal asked "Tasks wearing it stay; they just stop showing it. Delete calls? ")))))

(ert-deftest lumenna-menus-are-in-title-case-and-the-help-as-written ()
  (should (equal (lumenna--title-case "Fold or unfold what sits under this line")
                 "Fold or Unfold What Sits under This Line"))
  (should (equal (lumenna--title-case "Mark Done, or Mark Not Done") "Mark Done, or Mark Not Done"))
  (let ((menu (lumenna--menu 'lumenna-day-mode)))
    (should (assoc "The Day" (cdr menu)))
    (should (seq-find (lambda (item) (equal (aref item 0) "Add a Block")) (cdr (assoc "The Day" (cdr menu)))))))

;;;; Organising

(ert-deftest lumenna-a-project-is-made-renamed-archived-and-shows-its-tasks ()
  (lumenna-test--with-store
    (lumenna-projects)
    (lumenna-test--answering ("Wrok")
      (lumenna-project-add))
    (lumenna-test--goto "Wrok")
    (lumenna-test--answering ("Work")
      (lumenna-act-rename))
    (lumenna-test--goto "Work")
    (lumenna-act-archive)
    (lumenna-test--goto "Work")
    (should (string-match-p "archived" (lumenna-test--line)))
    ;; The answer is the whole line, so it includes the project the list began with.
    (lumenna-test--answering ("#Work ship it")
      (lumenna-project-add-task))
    (lumenna-test--goto "Work")
    (lumenna-activate)
    (should (equal (lumenna-test--titles "#Work") '("ship it")))
    (goto-char (point-min))
    (should (search-forward "ship it" nil t))))

(ert-deftest lumenna-labels-merge-and-filters-are-saved-and-requeried ()
  (lumenna-test--with-store
    (lumenna-test--seed "task.add" :text "ring the bank @calls")
    (lumenna-test--seed "label.add" :name "cals")
    (lumenna-labels)
    (lumenna-test--goto "cals")
    (lumenna-test--answering ("calls")
      (lumenna-act-merge))
    (should (equal (mapcar (lambda (r) (plist-get r :title))
                           (append (plist-get (lumenna-call "label.list") :rows) nil))
                   '("calls")))
    (lumenna-filters)
    (lumenna-test--answering ("Urgent" "p1")
      (lumenna-filter-add))
    (lumenna-test--goto "Urgent")
    (lumenna-test--answering ("p1 | p2")
      (lumenna-act-change-query))
    (should (equal (plist-get (aref (plist-get (lumenna-call "filter.list") :filters) 0) :query) "p1 | p2"))))

;;;; Typing

(ert-deftest lumenna-completion-offers-projects-and-counts-in-bytes ()
  (lumenna-test--with-store
    (lumenna-test--seed "project.add" :name "Work")
    (with-temp-buffer
      (insert "café #Wo")
      (let ((found (funcall (lumenna--completion "quick-add"))))
        (should found)
        (should (member "#Work" (nth 2 found)))
        (should (equal (buffer-substring (nth 0 found) (nth 1 found)) "#Wo"))))))

;;;; Keys and their help

(ert-deftest lumenna-every-key-a-help-lists-is-bound-to-its-command-there ()
  (let ((owners '(lumenna-command-map)))
    (mapatoms (lambda (symbol) (when (get symbol 'lumenna-keys) (push symbol owners))))
    (should (memq 'lumenna-devices-mode owners))
    (dolist (owner owners)
      ;; In a buffer in that mode: a derived mode's map takes its parent's keys
      ;; only once the mode has run.
      (with-temp-buffer
        (let ((map (if (fboundp owner) (progn (funcall owner) (current-local-map)) (symbol-value owner))))
          (dolist (group (lumenna--key-groups owner))
            (dolist (binding (cdr group))
              (should (equal (list owner (car binding) (keymap-lookup map (car binding)))
                             (list owner (car binding) (nth 2 binding)))))))))))

(ert-deftest lumenna-every-command-a-help-lists-is-in-the-menu-bar-too ()
  (let (modes)
    (mapatoms (lambda (symbol) (when (and (get symbol 'lumenna-keys) (fboundp symbol)) (push symbol modes))))
    (dolist (mode modes)
      (with-temp-buffer
        (funcall mode)
        (dolist (group (lumenna--key-groups mode))
          (dolist (binding (cdr group))
            (should (equal (list mode (nth 2 binding))
                           (list mode (and (seq-find (lambda (keys) (equal (aref keys 0) 'menu-bar))
                                                     (where-is-internal (nth 2 binding) (current-local-map)))
                                           (nth 2 binding)))))))))
    (dolist (group (get 'lumenna-command-map 'lumenna-keys))
      (dolist (binding (cdr group))
        (should (seq-find (lambda (keys) (equal (seq-take keys 3) [menu-bar tools lumenna]))
                          (where-is-internal (nth 2 binding) global-map)))))))

(ert-deftest lumenna-help-lists-a-lists-keys-once-and-runs-one-there ()
  (lumenna-test--with-store
    (lumenna-test--seed "task.add" :text "buy milk")
    (lumenna-tasks)
    (lumenna-test--goto "buy milk")
    (let ((list (current-buffer)))
      (lumenna-help)
      (should (derived-mode-p 'lumenna-help-mode))
      (goto-char (point-min))
      (should (string-match-p "\\`Keys in Lumenna: Tasks\\." (lumenna-test--line)))
      ;; The list's own RET, not the one every list has; and each key once.
      (should (= (how-many "^  RET: ") 1))
      (lumenna-test--goto "RET: Details")
      (lumenna-test--goto "Every list")
      (lumenna-test--goto "  u: Undo")
      (lumenna-test--goto "  c: Mark Done, or Mark Not Done")
      (lumenna-activate)
      (should (eq (current-buffer) list))
      (should (equal (lumenna-test--titles) nil)))))

;;;; Emacs's own conventions

(ert-deftest lumenna-the-minibuffer-reads-a-line-back-before-it-is-entered ()
  (lumenna-test--with-store
    (lumenna-test--seed "task.add" :text "ring the bank p1")
    (let (said)
      (cl-letf (((symbol-function 'minibuffer-contents-no-properties) (lambda () "call mum tomorrow p2 @calls"))
                (lumenna-announce-function (lambda (text _) (setq said text))))
        (with-temp-buffer
          (setq lumenna--syntax "quick-add")
          (lumenna-read-back)
          (should (string-match-p "\\`call mum, due tomorrow.*priority 2.*new label calls" said))
          (cl-letf (((symbol-function 'minibuffer-contents-no-properties) (lambda () "p1")))
            (setq lumenna--syntax "filter")
            (lumenna-read-back)
            (should (string-match-p "1 task" said))))))))

(ert-deftest lumenna-a-list-takes-emacs-undo-as-the-stores-and-goes-to-items-by-imenu ()
  (lumenna-test--with-store
    (lumenna-test--seed "task.add" :text "buy milk")
    (lumenna-test--seed "task.add" :text "post the letter")
    (lumenna-tasks)
    (should (eq (key-binding (kbd "C-/")) 'lumenna-undo))
    (should (eq (command-remapping 'undo-redo) 'lumenna-redo))
    (let ((index (funcall imenu-create-index-function)))
      (goto-char (cdr (assoc "post the letter" index)))
      (should (equal (lumenna-test--line) "post the letter")))
    (call-interactively (key-binding (kbd "C-/")))
    (should (equal (lumenna-test--titles) '("buy milk")))))

(ert-deftest lumenna-a-lists-commands-are-marked-for-it-and-global-ones-are-not ()
  (should (memq 'lumenna-tasks-mode (function-get 'lumenna-act-done 'command-modes)))
  (should (memq 'lumenna-task-mode (function-get 'lumenna-act-done 'command-modes)))
  (should (memq 'lumenna-list-mode (function-get 'lumenna-refresh 'command-modes)))
  (dolist (global '(lumenna-undo lumenna-redo lumenna lumenna-search lumenna-add lumenna-today))
    (should-not (function-get global 'command-modes))))

;;;; Settings and connecting

(ert-deftest lumenna-a-time-setting-reads-and-takes-hours-and-minutes ()
  (lumenna-test--with-store
    (lumenna-settings)
    (lumenna-test--goto "Day starts")
    (should (string-match-p "Day starts, 08:00" (lumenna-test--line)))
    (lumenna-test--answering ("9:30am")
      (lumenna-activate))
    (lumenna-test--goto "Day starts, 09:30")))

(ert-deftest lumenna-a-setting-offers-the-options-the-core-gives-it ()
  (lumenna-test--with-store
    (lumenna-settings)
    (lumenna-test--goto "Automatic backups")
    (should (equal (lumenna-test--line) "Automatic backups, Every day"))
    (let (offered)
      (cl-letf (((symbol-function 'completing-read)
                 (lambda (_prompt choices &rest _) (setq offered (mapcar #'car choices)) "Every week")))
        (lumenna-activate))
      (should (equal offered '("Every 12 hours" "Every day" "Every week" "Off"))))
    (lumenna-test--goto "Automatic backups, Every week")))

(defun lumenna-test--until (done)
  "Wait until DONE, a function of no arguments, is true: what `lum daemon'
answers comes when it comes."
  (let ((deadline (+ (float-time) 20)))
    (while (and (not (funcall done)) (< (float-time) deadline))
      (accept-process-output nil 0.05))
    (should (funcall done))))

(defun lumenna-test--has-line (text)
  "Whether this buffer has a line that is TEXT."
  (save-excursion
    (goto-char (point-min))
    (re-search-forward (concat "^" (regexp-quote text) "$") nil t)))

(defmacro lumenna-test--with-daemon (&rest body)
  "Run BODY against a fresh store, `lum daemon' stood in for by a script whose
service is the file `installed'; `said' collects what was announced.  The
script refuses to install while the file `refuse' exists, as `lum' would say."
  (declare (indent 0))
  `(lumenna-test--with-store
     (let* ((installed (expand-file-name "installed" profile))
            (refuse (expand-file-name "refuse" profile))
            (lumenna--daemon-program (expand-file-name "lum-daemon" profile))
            said
            (lumenna-announce-function (lambda (announcement _notices) (push announcement said))))
       (skip-unless (not (eq system-type 'windows-nt)))
       (with-temp-file lumenna--daemon-program
         (insert "#!/bin/sh\n"
                 "for verb; do :; done\n"
                 "case \"$verb\" in\n"
                 "  install) if [ -e '" refuse "' ]; then echo 'lum: launchctl refused' >&2; exit 1; fi\n"
                 "           touch '" installed "' ;;\n"
                 "  uninstall) rm -f '" installed "' ;;\n"
                 "esac\n"
                 "if [ -e '" installed "' ]; then on=true; else on=false; fi\n"
                 "printf '{\"version\": 1, \"result\": \"change\", \"announcement\": \"\", \"changed\": false, "
                 "\"service\": {\"installed\": %s, \"running\": false}}\\n' $on\n"))
       (set-file-modes lumenna--daemon-program #o755)
       ,@body)))

(ert-deftest lumenna-sync-in-the-background-is-a-setting-as-lum-finds-the-daemon ()
  (lumenna-test--with-store
    (lumenna-settings)
    ;; The real `lum', asked only whether it is installed: on a fresh profile it is not.
    (lumenna-test--until (lambda () (lumenna-test--has-line "Sync in the background, Off")))))

(ert-deftest lumenna-sync-in-the-background-installs-and-removes-the-daemon-and-says-the-cores-sentence ()
  (lumenna-test--with-daemon
    (lumenna-settings)
    (lumenna-test--until (lambda () (lumenna-test--has-line "Sync in the background, Off")))
    (lumenna-test--goto "Sync in the background")
    (lumenna-test--answering ("On")
      (lumenna-activate))
    (lumenna-test--until (lambda () (member "The sync daemon is on. It starts when you log in" said)))
    (should (file-exists-p installed))
    (should (lumenna-test--has-line "Sync in the background, On"))
    (lumenna-test--goto "Sync in the background")
    (lumenna-test--answering ("Off")
      (lumenna-activate))
    (lumenna-test--until (lambda () (member "The sync daemon is off" said)))
    (should-not (file-exists-p installed))
    (should (lumenna-test--has-line "Sync in the background, Off"))))

(ert-deftest lumenna-sync-in-the-background-refused-says-why-and-stays-off ()
  (lumenna-test--with-daemon
    (with-temp-file refuse)
    (lumenna-settings)
    (lumenna-test--until (lambda () (lumenna-test--has-line "Sync in the background, Off")))
    (lumenna-test--goto "Sync in the background")
    (lumenna-test--answering ("On")
      (lumenna-activate))
    (lumenna-test--until (lambda () (member "That could not be changed: launchctl refused" said)))
    (should-not (file-exists-p installed))
    (should (lumenna-test--has-line "Sync in the background, Off"))))

(ert-deftest lumenna-a-tasks-priority-is-chosen-by-the-cores-words ()
  (lumenna-test--with-store
    (lumenna-test--seed "task.add" :text "file taxes")
    (lumenna-tasks)
    (lumenna-test--goto "file taxes")
    (lumenna-activate)
    (lumenna-test--goto "Priority: Priority 4, none")
    (lumenna-test--answering ("Priority 1, highest")
      (lumenna-activate))
    (lumenna-test--goto "Priority: Priority 1, highest")))

(ert-deftest lumenna-the-places-are-the-cores-and-a-heading-adds-one ()
  (lumenna-test--with-store
    (lumenna)
    (lumenna-test--goto "Projects")
    (lumenna-test--answering ("Garden")
      (lumenna-test--act-named "New Project"))
    (lumenna-test--goto "  Garden")
    (lumenna-activate)
    (should (derived-mode-p 'lumenna-tasks-mode))))

(ert-deftest lumenna-devices-and-sync-is-the-first-line-of-settings ()
  (lumenna-test--with-store
    (lumenna-settings)
    (goto-char (point-min))
    (lumenna-test--goto "Devices and sync")
    (should-not (save-excursion (goto-char (point-min)) (re-search-forward "^clock" nil t)))
    (lumenna-activate)
    (should (derived-mode-p 'lumenna-devices-mode))))

(ert-deftest lumenna-an-empty-code-is-refused-and-the-kill-ring-is-never-read ()
  (lumenna-test--with-store
    (let ((kill-ring nil) (kill-ring-yank-pointer nil) (interprogram-paste-function nil))
      (kill-new "theirs123")
      (lumenna-test--answering ("")
        (should (equal (cadr (should-error (lumenna--read-code) :type 'user-error))
                       "Type or paste the code the other device shows")))
      (lumenna-test--answering ("typed456")
        (should (equal (lumenna--read-code) "typed456"))))))

(ert-deftest lumenna-a-tasks-project-is-chosen-from-the-projects-with-their-level ()
  (lumenna-test--with-store
    (lumenna-test--seed "project.add" :name "Work")
    (lumenna-test--seed "project.add" :name "Reports" :parent "Work")
    (lumenna-test--seed "task.add" :text "file the summary")
    (lumenna-tasks)
    (lumenna-test--goto "file the summary")
    (lumenna-activate)
    (lumenna-test--goto "Project:")
    (let (offered annotate required)
      (cl-letf (((symbol-function 'completing-read)
                 (lambda (_prompt collection _predicate require-match &rest _)
                   (setq offered (mapcar #'car collection)
                         required require-match
                         annotate (plist-get completion-extra-properties :annotation-function))
                   "Reports")))
        (lumenna-activate))
      (should (equal offered '("Inbox" "Work" "Reports")))
      (should (eq required t))
      (should (equal (funcall annotate "Reports") "  level 2"))
      (should (equal (funcall annotate "Work") "")))
    (lumenna-test--goto "Project: Reports")))

(ert-deftest lumenna-a-task-in-an-archived-project-keeps-it-on-the-project-list ()
  (lumenna-test--with-store
    (lumenna-test--seed "project.add" :name "Old")
    (lumenna-test--seed "task.add" :text "dusty #Old")
    (lumenna-test--seed "project.archive" :name "Old")
    (lumenna-tasks)
    (lumenna-test--goto "dusty")
    (lumenna-activate)
    (lumenna-test--goto "Project:")
    (let (offered)
      (cl-letf (((symbol-function 'completing-read)
                 (lambda (_prompt collection &rest _) (setq offered (mapcar #'car collection)) "Old")))
        (lumenna-activate))
      (should (equal offered '("Inbox" "Old"))))))

(ert-deftest lumenna-the-pairing-code-is-copied-only-when-asked ()
  (lumenna-test--with-store
    (let ((kill-ring nil) (kill-ring-yank-pointer nil) (interprogram-cut-function nil)
          (lumenna--code-shown nil))
      (lumenna-devices)
      (let ((turn (list :screen lumenna--screen
                        :view (plist-put (copy-sequence lumenna--view) :devices
                                         (list :status "" :syncing :json-false
                                               :pairing (list :name "here" :theirs "" :mine "abc123"
                                                              :running t :waiting t))))))
        (setq lumenna--view (plist-get turn :view))
        (lumenna--pairing-turned turn))
      (should (null kill-ring))
      (lumenna-pair-copy-code)
      (should (equal (car kill-ring) "abc123")))))

(ert-deftest lumenna-a-running-daemon-is-used-over-its-socket ()
  (skip-unless (not (eq system-type 'windows-nt)))
  (lumenna-test--with-store
    (let ((daemon (start-process "lumenna-daemon" nil lumenna-lum-program
                                 "--profile" lumenna-profile "sync-daemon" "--local-only")))
      (unwind-protect
          (progn
            (with-timeout (20 (ert-fail "the daemon made no socket"))
              (while (not (lumenna--socket)) (sleep-for 0.1)))
            (lumenna-test--seed "task.add" :text "through the daemon")
            (should (eq (process-type (jsonrpc--process lumenna--connection)) 'network))
            (should (equal (lumenna-test--titles) '("through the daemon"))))
        (lumenna-disconnect)
        (delete-process daemon)))))

;;;; The planner

(defun lumenna-test--block-at (title kind at minutes date)
  (lumenna-test--seed "block.add" :title title :kind kind :at at :minutes minutes :date date))

(defmacro lumenna-test--planning (now &rest body)
  "Run BODY against a fresh store, proposals asked as at NOW, times in 24 hours.
`said' collects what was announced."
  (declare (indent 1))
  `(lumenna-test--with-store
     (let* ((lumenna-proposal--now (lambda () ,now))
            (display-time-24hr-format t)
            said
            (lumenna-announce-function (lambda (announcement notices) (push (cons announcement notices) said))))
       ,@body)))

(defun lumenna-test--lines ()
  "Every line of the buffer, without its properties."
  (split-string (buffer-substring-no-properties (point-min) (point-max)) "\n" t))

(ert-deftest lumenna-the-days-heading-offers-the-days-own-actions-and-what-to-work-on-says-why ()
  (lumenna-test--with-store
    (lumenna-test--seed "task.add" :text "write the chapter p1 30m")
    (lumenna-test--seed "block.add" :title "Focus" :at "00:00" :minutes 1440 :date "today")
    (lumenna-today)
    (goto-char (point-min))
    (let ((kinds (mapcar (lambda (a) (plist-get a :kind)) (lumenna-actions))))
      (should (equal (seq-take kinds 4) '("plan_day" "replan" "what_now" "end_day"))))
    (let (annotated)
      (cl-letf (((symbol-function 'completing-read)
                 (lambda (_prompt choices &rest _)
                   (let ((line (car (car choices))))
                     (setq annotated (funcall (plist-get completion-extra-properties :annotation-function) line))
                     (should-not (string-match-p "Suggested" line))
                     line))))
        (lumenna-day-what-now))
      (should (string-match-p "\\`  Suggested because priority 1" annotated)))
    (lumenna-test--goto "write the chapter")
    (should (string-match-p "in progress" (lumenna-test--line)))))

(ert-deftest lumenna-a-day-without-an-action-says-the-cores-sentence ()
  (lumenna-test--with-store
    (lumenna-today)
    (lumenna-day-previous)
    (should (equal (cadr (should-error (lumenna-day-replan) :type 'user-error))
                   "That does not apply here."))))

(ert-deftest lumenna-replan-says-its-proposal-and-shows-the-next-after-a-partial-apply ()
  (lumenna-test--planning "2030-01-15T10:40"
    (lumenna-test--block-at "Deep work" "work" "09:00" 120 "2030-01-15")
    (lumenna-test--block-at "Lunch" "break" "10:30" 30 "2030-01-15")
    (lumenna-test--block-at "Review" "work" "11:00" 60 "2030-01-15")
    (setq said nil)
    (lumenna-replan)
    (should (equal (caar said)
                   "Lunch moves from 10:30 to 11:00; Review moves from 11:00 to 11:30 and shortens from 1 hour to 30 minutes."))
    (should (equal (lumenna-test--lines)
                   '("Replan from Now. Lunch moves from 10:30 to 11:00; Review moves from 11:00 to 11:30 and shortens from 1 hour to 30 minutes."
                     "Lunch moves from 10:30 to 11:00, checked"
                     "Review moves from 11:00 to 11:30 and shortens from 1 hour to 30 minutes, checked"
                     "Make the Changes Checked. A change left unchecked is not made: what it would move stays where it is, and the rest of the day is proposed again around it."
                     "Other options"
                     "  Let Lunch shorten from 30 minutes to 15 minutes: 15 minutes less break. Then Review moves from 11:00 to 11:15 and shortens from 1 hour to 45 minutes."
                     "  Skip Lunch today, your only break left today. Then nothing else needs to change."
                     "  Skip Review today. Then Lunch moves from 10:30 to 11:00."
                     "Cancel")))
    (lumenna-test--goto "30 minutes, checked")
    (lumenna-proposal-toggle)
    (should (equal (caar said) "Make this change, not checked"))
    (should (string-suffix-p ", not checked" (lumenna-test--line)))
    (lumenna-test--goto "Make the Changes")
    (lumenna-activate)
    (should (equal (caar said) "Replanned: Lunch moves from 10:30 to 11:00. Lunch moves from 11:00 to 12:00."))
    ;; Lunch moved alone runs into Review, kept where it is: the core proposes again.
    (should (equal (buffer-name) "*Lumenna: Replan from Now*"))
    (should (member "Lunch moves from 11:00 to 12:00, checked" (lumenna-test--lines)))
    (lumenna-proposal-apply)
    (should-not (get-buffer "*Lumenna: Replan from Now*"))
    (let ((plan (lumenna-call "plan" :date "2030-01-15")))
      (should (equal (mapcar (lambda (b) (cons (plist-get b :title) (plist-get b :start)))
                             (append (plist-get plan :blocks) nil))
                     '(("Deep work" . "09:00") ("Review" . "11:00") ("Lunch" . "12:00")))))))

(ert-deftest lumenna-a-replan-with-nothing-to-change-says-so-with-no-button-and-a-stale-one-is-refused ()
  (lumenna-test--planning "2030-01-15T10:40"
    (lumenna-test--block-at "Deep work" "work" "09:00" 120 "2030-01-15")
    (lumenna-test--block-at "Lunch" "break" "10:30" 30 "2030-01-15")
    (lumenna-replan)
    ;; Meanwhile the day moves: the same proposal is made elsewhere.
    (let ((elsewhere (lumenna-call "replan" :clock "24-hour" :now "2030-01-15T10:40")))
      (lumenna-test--seed "replan.apply" :proposal elsewhere
                          :accepted (vconcat (plist-get elsewhere :changes)) :now "2030-01-15T10:40"))
    (setq said nil)
    (lumenna-proposal-apply)
    (let ((lines (lumenna-test--lines)))
      (should (string-match-p "the day has changed since this was proposed" (nth 1 lines)))
      (should (equal (nth 2 lines) "Nothing needs to change: the rest of the day still fits."))
      (should-not (seq-find (lambda (l) (string-prefix-p "Make the Changes" l)) lines))
      (should (equal (car (last lines)) "Cancel")))))

(ert-deftest lumenna-plan-the-day-gives-each-sitting-its-reason-and-plans-again-around-what-was-turned-down ()
  (lumenna-test--planning "2030-01-15T08:00"
    (lumenna-test--block-at "Deep work" "work" "09:00" 60 "2030-01-15")
    (lumenna-test--block-at "Admin" "work" "14:00" 60 "2030-01-15")
    (lumenna-test--seed "task.add" :text "write report p1 15m")
    (lumenna-test--seed "task.add" :text "email bob 30m")
    (lumenna-plan-the-day "2030-01-15")
    (should (equal (caar said) "Plans 2 tasks: write report and email bob in Deep work."))
    (should (member "write report goes in Deep work at 09:00, for 15 minutes, checked, Suggested because priority 1 and its estimate of 15 minutes fits the 1 hour left."
                    (lumenna-test--lines)))
    (lumenna-test--goto "email bob goes in")
    (lumenna-activate)
    (lumenna-proposal-apply)
    (should (equal (caar said) "Planned 1 task. Now: Plans 1 task: email bob in Admin."))
    (lumenna-test--goto "email bob goes in Admin at 14:00")
    (lumenna-proposal-apply)
    (should-not (get-buffer "*Lumenna: Plan the Day*"))
    (let ((blocks (append (plist-get (lumenna-call "plan" :date "2030-01-15") :blocks) nil)))
      (should (equal (mapcar (lambda (b) (mapcar (lambda (a) (plist-get a :title)) (plist-get b :assignments))) blocks)
                     '(("write report") ("email bob")))))))

(ert-deftest lumenna-plan-the-day-is-the-days-form-and-cancel-makes-nothing ()
  (lumenna-test--with-store
    (lumenna-test--seed "task.add" :text "write report 15m")
    (lumenna-test--seed "block.add" :title "Deep" :at "9:00" :minutes 60 :date "tomorrow")
    (lumenna-today)
    (lumenna-day-next)
    (lumenna-day-plan)
    (should (equal (buffer-name) "*Lumenna: Plan the Day*"))
    (lumenna-proposal-cancel)
    (should-not (get-buffer "*Lumenna: Plan the Day*"))
    (should (equal (plist-get (aref (plist-get (lumenna-call "plan" :date "tomorrow") :blocks) 0) :assignments) []))))

(ert-deftest lumenna-a-tasks-history-is-the-last-line-of-its-own-screen-and-never-in-a-list ()
  (lumenna-test--with-store
    (lumenna-test--seed "task.add" :text "write the chapter")
    (lumenna-tasks)
    (should-not (seq-find (lambda (l) (string-match-p "made today" l)) (lumenna-test--lines)))
    (lumenna-test--goto "write the chapter")
    (lumenna-activate)
    (let ((lines (lumenna-test--lines)))
      (should (equal (car (last lines)) "History, made today"))
      ;; Its state is the core's line, its label then what it says.
      (should (seq-find (lambda (l) (string-prefix-p "State, unassigned" l)) lines)))))

(ert-deftest lumenna-a-blocks-end-offers-to-carry-its-short-work-forward-from-the-notification ()
  (lumenna-test--reminding
    (lumenna-call "config.set" :key "default-block-reminders" :value "")
    (lumenna-test--seed "block.add" :title "Deep work" :at "9:00" :minutes 60 :date "2030-01-15")
    (lumenna-test--seed "block.add" :title "Admin" :at "9:00" :minutes 60 :date "2030-01-16")
    (lumenna-test--seed "task.add" :text "Essay")
    (let* ((plan (lumenna-call "plan" :date "2030-01-15"))
           (block (plist-get (aref (plist-get plan :blocks) 0) :id))
           (task (plist-get (aref (plist-get (lumenna-call "task.list") :rows) 0) :id)))
      (lumenna-call "assign" :task task :block block :date "2030-01-15" :minutes 90)
      (let ((sitting (plist-get (aref (plist-get (aref (plist-get (lumenna-call "plan" :date "2030-01-15") :blocks) 0)
                                                 :assignments)
                                      0)
                                :id)))
        (lumenna-call "stop" :assignment sitting :minutes 40)))
    (setq now "2030-01-15T09:58:00Z")
    (lumenna-test--deliver)
    (setq now "2030-01-15T10:00:30Z" said nil)
    (lumenna-test--deliver)
    (should (equal said '("Deep work is ending. Essay: 40 minutes of 1 hour 30 minutes. Carry the remaining 50 minutes to Admin tomorrow?")))
    (let ((notification (car shown)))
      (should (equal (plist-get notification :actions)
                     '("carry" "Carry Forward" "dismiss" "Dismiss" "snooze" "Snooze 10 Minutes" "default" "Deep work")))
      (funcall (plist-get notification :on-action) 1 "carry"))
    (should (equal (car said) "Carried forward Essay to Admin"))
    (should-not (lumenna-test--due))
    (let ((next (aref (plist-get (lumenna-call "plan" :date "2030-01-16") :blocks) 0)))
      (should (equal (plist-get (aref (plist-get next :assignments) 0) :planned_mins) 50)))))

(ert-deftest lumenna-the-due-list-carries-forward-only-where-a-reminder-offers-to ()
  (lumenna-test--reminding
    (lumenna-test--seed "block.add" :title "Deep work" :at "9:00" :minutes 60 :date "2030-01-15")
    (lumenna-test--deliver)
    (setq now "2030-01-15T08:55:30Z")
    (lumenna-test--deliver)
    (lumenna-reminders-due)
    (lumenna-test--goto "Deep work starts")
    ;; The app's list: a reminder without the button refuses its key in the core's words.
    (should (equal (cadr (should-error (lumenna-reminders-carry) :type 'user-error))
                   "That does not apply here."))
    (should (equal (cadr (should-error (lumenna-reminders-replan) :type 'user-error))
                   "That does not apply here."))
    (should (equal (length (lumenna-test--due)) 1))))

(ert-deftest lumenna-replan-offers-other-options-each-proposed-in-place-when-taken ()
  (lumenna-test--planning "2030-01-15T10:40"
    (lumenna-test--block-at "Deep work" "work" "09:00" 120 "2030-01-15")
    (lumenna-test--block-at "Lunch" "break" "10:30" 30 "2030-01-15")
    (lumenna-test--block-at "Review" "work" "11:00" 60 "2030-01-15")
    (lumenna-replan)
    (let ((lines (lumenna-test--lines)))
      (should (equal (seq-drop (member "Other options" lines) 1)
                     '("  Let Lunch shorten from 30 minutes to 15 minutes: 15 minutes less break. Then Review moves from 11:00 to 11:15 and shortens from 1 hour to 45 minutes."
                       "  Skip Lunch today, your only break left today. Then nothing else needs to change."
                       "  Skip Review today. Then Lunch moves from 10:30 to 11:00."
                       "Cancel"))))
    (lumenna-test--goto "Skip Lunch today")
    (lumenna-activate)
    (should (equal (caar said) "Lunch is skipped today."))
    (let ((lines (lumenna-test--lines)))
      (should (equal (nth 1 lines) "Lunch is skipped today, checked"))
      (should-not (member "Other options" lines)))))

(ert-deftest lumenna-a-blocks-work-outrunning-it-offers-replan-from-the-notification ()
  (lumenna-test--reminding
    (lumenna-call "config.set" :key "default-block-reminders" :value "")
    (lumenna-test--seed "block.add" :title "Deep work" :at "9:00" :minutes 60 :date "2030-01-15")
    (lumenna-test--seed "block.add" :title "Meeting" :at "10:00" :minutes 60 :date "2030-01-15")
    (let ((block (plist-get (aref (plist-get (lumenna-call "plan" :date "2030-01-15") :blocks) 0) :id)))
      (dolist (task '(("Essay" . 40) ("Email" . 30)))
        (let ((id (plist-get (plist-get (lumenna-call "task.add" :text (car task)) :task) :id)))
          (lumenna-call "assign" :task id :block block :date "2030-01-15" :minutes (cdr task)))))
    (setq now "2030-01-15T08:58:00Z")
    (lumenna-test--deliver)
    (setq now "2030-01-15T09:00:30Z" said nil)
    (lumenna-test--deliver)
    (should (equal said '("Deep work has 1 hour left and 1 hour 10 minutes of work planned, more than it holds")))
    (let ((notification (car shown))
          (lumenna-proposal--now (lambda () "2030-01-15T09:01")))
      (should (equal (plist-get notification :actions)
                     '("replan" "Replan" "dismiss" "Dismiss" "snooze" "Snooze 10 Minutes" "default" "Deep work")))
      (funcall (plist-get notification :on-action) 1 "replan")
      (should (equal (buffer-name) "*Lumenna: Replan from Now*"))
      (lumenna-reminders-due)
      (lumenna-test--goto "Deep work has")
      (lumenna-reminders-replan)
      (should (equal (buffer-name) "*Lumenna: Replan from Now*")))))

(ert-deftest lumenna-a-colour-is-a-blank-mark-in-its-colour ()
  "A colour is spaces in its colour, blank to speech, and nothing without colour."
  (let ((teal '(:name "teal" :light "#00796B" :dark "#4DB6AC")))
    (cl-letf (((symbol-function 'display-color-p) (lambda (&rest _) t))
              ((symbol-function 'frame-parameter)
               (lambda (_frame parameter) (and (eq parameter 'background-mode) 'dark))))
      (let ((mark (lumenna--swatch teal)))
        (should (equal (substring-no-properties mark) "   "))
        (should (equal (plist-get (get-text-property 0 'face mark) :background) "#4DB6AC"))))
    (cl-letf (((symbol-function 'display-color-p) (lambda (&rest _) nil)))
      (should (equal (lumenna--swatch teal) "")))
    (should (equal (lumenna--swatch nil) ""))))

;;;; Connections, keys and exports

(ert-deftest lumenna-every-buffer-draws-its-screen-again-over-a-new-connection ()
  (lumenna-test--with-store
    (lumenna-test--seed "task.add" :text "before")
    (lumenna)
    (lumenna-tasks)
    (let ((tasks (current-buffer)))
      ;; The server goes, as a daemon does when the background setting reinstalls it.
      (jsonrpc-shutdown lumenna--connection)
      (lumenna-test--seed "task.add" :text "after")
      (with-current-buffer tasks
        (goto-char (point-min))
        (should (search-forward "before" nil t))
        (should (search-forward "after" nil t)))
      (with-current-buffer "*Lumenna*"
        (goto-char (point-min))
        (should (search-forward "Inbox" nil t))))))

(ert-deftest lumenna-disconnecting-stops-the-reminders-until-lumenna-connects-again ()
  (lumenna-test--with-store
    (let ((lumenna-reminders-mode nil)
          (lumenna-reminders--timer nil)
          (lumenna-reminders--asking nil)
          (lumenna-reminders--resume nil))
      (unwind-protect
          (progn
            (lumenna-call "task.list")
            (lumenna-reminders-mode 1)
            (should lumenna-reminders--timer)
            (lumenna-disconnect)
            (should-not lumenna-reminders-mode)
            (should-not lumenna-reminders--timer)
            (should-not (advice-function-member-p #'lumenna-reminders--focus-changed after-focus-change-function))
            (should-not lumenna--connection)
            (lumenna-call "task.list")
            (should lumenna-reminders-mode))
        (lumenna-reminders-mode -1)))))

(ert-deftest lumenna-a-new-saved-filters-query-is-read-in-the-filter-language ()
  (lumenna-test--with-store
    (let (read)
      (cl-letf (((symbol-function 'lumenna-read-line)
                 (lambda (prompt syntax &rest _) (push (cons prompt syntax) read) "p1"))
                ((symbol-function 'read-from-minibuffer) (lambda (&rest _) "Urgent"))
                ((symbol-function 'read-string) (lambda (&rest _) "Urgent")))
        (lumenna-filters)
        (lumenna-filter-add))
      (should (equal read '(("New saved filter. A filter, such as p1 & due before: friday. Query: " . "filter")))))
    (should (equal (plist-get (aref (plist-get (lumenna-call "filter.list") :filters) 0) :query) "p1"))))

(ert-deftest lumenna-d-on-a-trashed-tasks-details-deletes-it-from-the-trash ()
  (lumenna-test--with-store
    (let ((id (plist-get (plist-get (lumenna-test--seed "task.add" :text "gone soon") :task) :id)))
      (lumenna-test--seed "task.rm" :id id)
      (lumenna-task-show id)
      (lumenna-test--answering (t)
        (lumenna-task-delete))
      (should (equal (lumenna-test--titles "deleted") nil)))))

(ert-deftest lumenna-a-project-and-a-saved-filter-take-a-colour-by-its-key-as-a-label-does ()
  (dolist (map (list lumenna-projects-mode-map lumenna-labels-mode-map lumenna-filters-mode-map))
    (should (eq (keymap-lookup map "C") #'lumenna-act-colour))))

(ert-deftest lumenna-an-export-is-one-the-app-offers-saved-where-the-person-says ()
  (lumenna-test--with-store
    (lumenna-test--seed "task.add" :text "exported")
    (lumenna-settings)
    (let ((file (expand-file-name "out.md" lumenna-profile))
          offered)
      (cl-letf (((symbol-function 'completing-read)
                 (lambda (_prompt choices &rest _)
                   (setq offered (mapcar #'cdr choices))
                   (car (rassoc "markdown" choices))))
                ((symbol-function 'read-file-name) (lambda (&rest _) file)))
        (lumenna-export)
        ;; Where to keep it is asked once the turn that made it has been taken.
        (let ((deadline (+ (float-time) 10)))
          (while (and (not (file-exists-p file)) (< (float-time) deadline))
            (accept-process-output nil 0.05))))
      (should (equal offered '("json" "markdown" "org" "ics")))
      (with-temp-buffer
        (insert-file-contents file)
        (should (search-forward "exported" nil t))))))

(provide 'lumenna-test)

;;; lumenna-test.el ends here
