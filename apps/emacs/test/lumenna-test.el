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

;;;; Tasks

(ert-deftest lumenna-a-weight-is-read-by-the-core-which-refuses-a-typo ()
  (lumenna-test--with-store
    (lumenna-write "project.add" :name "Work")
    (lumenna-projects)
    (lumenna-test--goto "Work")
    (lumenna-test--answering ("1,5")
      (should-error (lumenna-act-weight) :type 'user-error))
    (lumenna-test--answering ("1.5")
      (lumenna-act-weight))
    (lumenna-test--goto "Work")
    (should (string-match-p "1.5" (lumenna-test--line)))))

(ert-deftest lumenna-a-key-runs-only-what-the-row-offers ()
  (lumenna-test--with-store
    (lumenna-projects)
    (lumenna-test--goto "Inbox")
    ;; The Inbox keeps its name: the core offers no Rename, so the key refuses.
    (let ((refusal (should-error (lumenna-act-rename) :type 'user-error)))
      (should (equal (cadr refusal)
                     "The Inbox keeps its name and its place; only its order and weight change. This offers Weight")))
    (let (offered)
      (cl-letf (((symbol-function 'completing-read)
                 (lambda (_prompt choices &rest _) (setq offered (mapcar #'car choices)) (car offered)))
                ((symbol-function 'read-string) (lambda (&rest _) "1")))
        (lumenna-act-at-point))
      (should (equal offered '("Weight"))))))

(ert-deftest lumenna-a-project-with-a-space-is-quoted-by-the-core-in-its-list ()
  (lumenna-test--with-store
    (lumenna-write "project.add" :name "Home Office")
    (lumenna-write "task.add" :text "file the receipts #\"Home Office\"")
    (lumenna-projects)
    (lumenna-test--goto "Home Office")
    (lumenna-activate)
    (should (equal lumenna--prefix "#\"Home Office\" "))
    (should (equal (lumenna-test--titles "#\"Home Office\"") '("file the receipts")))
    (goto-char (point-min))
    (should (search-forward "file the receipts" nil t))))

(ert-deftest lumenna-a-pick-with-nothing-to-offer-says-the-cores-sentence ()
  (lumenna-test--with-store
    (lumenna-write "task.add" :text "only one")
    (lumenna-tasks)
    (lumenna-test--goto "only one")
    (let (said)
      (let ((lumenna-announce-function (lambda (text _) (setq said text))))
        (lumenna-test--answering ()
          (lumenna-act-wait-for)))
      (should (equal said "There is no task it could wait for.")))))

(ert-deftest lumenna-a-time-is-said-in-the-clock-the-mode-line-uses ()
  (let ((display-time-24hr-format nil))
    (should (equal (mapcar #'lumenna-time '("00:05" "09:00" "12:30" "15:00")) '("12:05 AM" "9:00 AM" "12:30 PM" "3:00 PM"))))
  (let ((display-time-24hr-format t))
    (should (equal (lumenna-time "15:00") "15:00"))))

(ert-deftest lumenna-a-task-due-at-a-time-says-it-in-emacss-clock-before-its-priority ()
  (lumenna-test--with-store
    (lumenna-write "task.add" :text "call the bank tomorrow at 3pm p1")
    (let ((display-time-24hr-format t))
      (lumenna-tasks)
      (lumenna-test--goto "call the bank")
      (should (equal (lumenna-test--line) "call the bank, due tomorrow at 15:00, priority 1")))))

(ert-deftest lumenna-a-task-is-listed-in-words-completed-and-brought-back-with-undo ()
  (lumenna-test--with-store
    (lumenna-write "task.add" :text "buy milk tomorrow")
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
    (lumenna-write "task.add" :text "paint the room")
    (lumenna-write "task.add" :text "buy paint")
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
    (lumenna-write "task.add" :text "water plants every monday")
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
    (lumenna-write "task.add" :text "keep me")
    (lumenna-write "task.add" :text "lose me")
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

;;;; The day

(ert-deftest lumenna-the-day-opens-on-now-and-plans-a-sitting ()
  (lumenna-test--with-store
    (lumenna-write "task.add" :text "write the chapter")
    (lumenna-write "block.add" :title "Focus" :at "00:00" :minutes 1439 :date "today")
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
    (lumenna-write "task.add" :text "write the chapter")
    (lumenna-write "block.add" :title "Focus" :at "9am" :minutes 90 :date "tomorrow")
    (lumenna-write "block.add" :title "Lunch" :at "noon" :minutes 30 :kind "break" :date "tomorrow")
    (lumenna-tasks)
    (lumenna-test--goto "write the chapter")
    (let (offered)
      (cl-letf (((symbol-function 'completing-read)
                 (lambda (_prompt choices &rest _)
                   (setq offered (mapcar #'car choices))
                   (car offered)))
                ((symbol-function 'read-from-minibuffer) (lambda (&rest _) "30")))
        (lumenna-act-put-in-block))
      (should (equal (length offered) 1))
      (should (string-match-p ", 9:00 AM to 10:30 AM, Focus\\'" (car offered))))
    (let ((plan (lumenna-call "plan" :date "tomorrow")))
      (should (equal (plist-get (aref (plist-get (aref (plist-get plan :blocks) 0) :assignments) 0) :planned_mins)
                     30)))))

(ert-deftest lumenna-a-sitting-pauses-resumes-and-stops ()
  (lumenna-test--with-store
    (lumenna-write "task.add" :text "write the chapter")
    (lumenna-write "block.add" :title "Focus" :at "00:00" :minutes 1439 :date "today")
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
    (lumenna-write "block.add" :title "Train" :at "00:00" :minutes 1439 :date "today" :kind "break")
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
    (lumenna-write "block.add" :title "Run" :at "7am" :minutes 30 :date "today" :repeat "every day")
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
    (lumenna-write "block.add" :title "Dentist" :at "2pm" :minutes 60 :kind "event" :date "tomorrow")
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
    (lumenna-write "block.add" :title "Run" :at "7am" :minutes 30 :date "today" :repeat "every day")
    (let* ((display-time-24hr-format t)
           (series (car (split-string (plist-get (aref (plist-get (lumenna-call "block.list") :rows) 0) :id) "@")))
           (_ (lumenna-call "block.cancel" :id series :date "tomorrow"))
           (rows (lumenna--day-rows (lumenna-call "plan" :date "tomorrow")))
           (line (lambda (role) (lumenna-describe (seq-find (lambda (r) (equal (plist-get r :role) role)) rows)))))
      (should (equal (funcall line "cancelled") "07:00, Run, cancelled for this day"))
      (should (string-match-p "\\`Free, [0-9]+ hours?\\( [0-9]+ minutes?\\)?, [0-9:]+ to [0-9:]+\\'" (funcall line "free")))
      (should (string-match-p "\\`Now, [0-9][0-9]:[0-9][0-9]\\'"
                              (funcall (lambda () (lumenna-describe (seq-find (lambda (r) (equal (plist-get r :role) "now"))
                                                                             (lumenna--day-rows (lumenna-call "plan")))))))))))

(ert-deftest lumenna-an-empty-list-says-the-cores-words-under-a-heading-without-its-count ()
  (lumenna-test--with-store
    (lumenna-blocks)
    (goto-char (point-min))
    (should (equal (lumenna-test--line) "Blocks"))
    (forward-line 1)
    (should (equal (lumenna-test--line) "No blocks."))
    (should-error (lumenna-row) :type 'user-error)
    (lumenna-filters)
    (should (equal (buffer-string) "Saved filters\nNo saved filters. A filter's query is kept here under a name.\n"))
    (lumenna-devices)
    (should (equal (buffer-string) "Devices and sync\nNo devices are paired yet. Pair one to sync with it.\n"))
    (lumenna-write "label.add" :name "calls")
    (lumenna-labels)
    (goto-char (point-min))
    (should (equal (lumenna-test--line) "Labels, 1 label"))))

(ert-deftest lumenna-going-to-a-day-and-a-new-filter-ask-the-cores-questions ()
  (lumenna-test--with-store
    (lumenna-write "block.add" :title "Dentist" :at "2pm" :minutes 60 :kind "event" :date "tomorrow")
    (lumenna-today)
    (let (asked (answers (list "tomorrow" "Urgent" "p1")))
      (cl-letf (((symbol-function 'read-from-minibuffer) (lambda (prompt &rest _) (push prompt asked) (pop answers)))
                ((symbol-function 'read-string) (lambda (prompt &rest _) (push prompt asked) (pop answers))))
        (call-interactively #'lumenna-day-go-to)
        (goto-char (point-min))
        (should (string-match-p "\\`[A-Z][a-z]+ [0-9]+ [A-Z][a-z]+ [0-9]+\\. 1 block" (lumenna-test--line)))
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

(ert-deftest lumenna-a-rule-the-words-cannot-say-is-noted-at-repeats ()
  (lumenna-test--with-store
    (lumenna-write "block.add" :title "Board" :at "6pm" :minutes 60 :date "today" :repeat "every day")
    (lumenna-blocks)
    (lumenna-test--goto "Board")
    (let ((real (symbol-function 'lumenna-call)) asked)
      (cl-letf (((symbol-function 'lumenna-call)
                 (lambda (method &rest params)
                   (let ((result (apply real method params)))
                     ;; As a rule from an import or another app arrives: no words for it.
                     (if (equal method "block.show")
                         (plist-put (plist-put (copy-sequence result) :rrule "FREQ=MONTHLY;BYDAY=2TU") :repetition nil)
                       result))))
                ((symbol-function 'completing-read) (lambda (&rest _) "Repeats"))
                ((symbol-function 'read-from-minibuffer) (lambda (prompt &rest _) (setq asked prompt) "")))
        (lumenna-act-edit))
      (should (equal asked "It repeats by the rule FREQ=MONTHLY;BYDAY=2TU, which the repetition words cannot say. Leave Repeats empty to keep it. Repeats: ")))))

(ert-deftest lumenna-a-destructive-question-says-what-it-does-then-asks-it ()
  (lumenna-test--with-store
    (lumenna-write "label.add" :name "calls")
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
    (lumenna-write "task.add" :text "ring the bank @calls")
    (lumenna-write "label.add" :name "cals")
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
    (lumenna-write "project.add" :name "Work")
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
    (lumenna-write "task.add" :text "buy milk")
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
    (lumenna-write "task.add" :text "ring the bank p1")
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
    (lumenna-write "task.add" :text "buy milk")
    (lumenna-write "task.add" :text "post the letter")
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

(ert-deftest lumenna-a-tasks-priority-is-chosen-by-the-cores-words ()
  (lumenna-test--with-store
    (lumenna-write "task.add" :text "file taxes")
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

(ert-deftest lumenna-an-empty-code-is-the-clipboards-but-never-this-devices-own ()
  (lumenna-test--with-store
    (let ((kill-ring nil) (kill-ring-yank-pointer nil) (interprogram-paste-function nil)
          (lumenna--shown-code nil))
      (kill-new "theirs123")
      (lumenna-test--answering ("")
        (should (equal (lumenna--read-code) "theirs123")))
      (setq lumenna--shown-code "theirs123")
      (lumenna-test--answering ("")
        (should (equal (cadr (should-error (lumenna--read-code) :type 'user-error))
                       "Type or paste the code the other device shows")))
      (lumenna-test--answering ("typed456")
        (should (equal (lumenna--read-code) "typed456"))))))

(ert-deftest lumenna-a-running-daemon-is-used-over-its-socket ()
  (skip-unless (not (eq system-type 'windows-nt)))
  (lumenna-test--with-store
    (let ((daemon (start-process "lumenna-daemon" nil lumenna-lum-program
                                 "--profile" lumenna-profile "sync-daemon" "--local-only")))
      (unwind-protect
          (progn
            (with-timeout (20 (ert-fail "the daemon made no socket"))
              (while (not (lumenna--socket)) (sleep-for 0.1)))
            (lumenna-write "task.add" :text "through the daemon")
            (should (eq (process-type (jsonrpc--process lumenna--connection)) 'network))
            (should (equal (lumenna-test--titles) '("through the daemon"))))
        (lumenna-disconnect)
        (delete-process daemon)))))

(provide 'lumenna-test)

;;; lumenna-test.el ends here
