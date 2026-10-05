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

(defun lumenna-test--line ()
  (buffer-substring-no-properties (line-beginning-position) (line-end-position)))

;;;; Tasks

(ert-deftest lumenna-a-task-is-listed-in-words-completed-and-brought-back-with-undo ()
  (lumenna-test--with-store
    (lumenna-write "task.add" :text "buy milk tomorrow")
    (lumenna-tasks)
    (lumenna-test--goto "buy milk")
    (should (string-match-p "\\`buy milk, due " (lumenna-test--line)))
    (lumenna-task-toggle-done)
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
    (lumenna-test--answering ("paint the room")
      (lumenna-task-make-subtask))
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
    (lumenna-task-delete)
    (lumenna-test--goto "lose me")
    (lumenna-task-delete)
    (lumenna-trash)
    (lumenna-test--goto "keep me")
    (lumenna-activate)
    (lumenna-test--goto "lose me")
    (lumenna-test--answering (t)
      (lumenna-trash-erase))
    (should (equal (lumenna-test--titles) '("keep me")))
    (should (equal (lumenna-test--titles "deleted") nil))))

;;;; The day

(ert-deftest lumenna-the-day-opens-on-now-and-plans-a-sitting ()
  (lumenna-test--with-store
    (lumenna-write "task.add" :text "write the chapter")
    (lumenna-write "block.add" :title "Focus" :at "00:00" :minutes 1439 :date "today")
    (lumenna-today)
    (should (string-match-p "Focus" (lumenna-test--line)))
    (lumenna-test--answering ("write the chapter" "45")
      (lumenna-day-assign))
    (lumenna-test--goto "write the chapter")
    (should (string-match-p "planned for 45 minutes" (lumenna-test--line)))
    (lumenna-day-timer)
    (lumenna-test--goto "write the chapter")
    (should (string-match-p "in progress, 45 minutes planned" (lumenna-test--line)))
    (lumenna-test--answering ("")
      (lumenna-day-planned-length))
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
                ((symbol-function 'read-string) (lambda (&rest _) "30")))
        (lumenna-task-assign))
      (should (equal (length offered) 1))
      (should (string-match-p ", 09:00 to 10:30, Focus\\'" (car offered))))
    (let ((plan (lumenna-call "plan" :date "tomorrow")))
      (should (equal (plist-get (aref (plist-get (aref (plist-get plan :blocks) 0) :assignments) 0) :planned_mins)
                     30)))))

(ert-deftest lumenna-a-sitting-pauses-resumes-and-stops ()
  (lumenna-test--with-store
    (lumenna-write "task.add" :text "write the chapter")
    (lumenna-write "block.add" :title "Focus" :at "00:00" :minutes 1439 :date "today")
    (lumenna-today)
    (lumenna-test--answering ("write the chapter" "")
      (lumenna-day-assign))
    (lumenna-test--goto "write the chapter")
    (lumenna-day-timer)
    (lumenna-test--goto "write the chapter")
    (should (string-match-p "in progress" (lumenna-test--line)))
    (lumenna-day-timer)
    (lumenna-test--goto "write the chapter")
    (should (string-match-p "paused" (lumenna-test--line)))
    (lumenna-day-timer)
    (lumenna-test--goto "write the chapter")
    (should (string-match-p "in progress" (lumenna-test--line)))
    (lumenna-day-stop)
    (lumenna-test--goto "write the chapter")
    (should (string-match-p "worked" (lumenna-test--line)))))

(ert-deftest lumenna-a-block-is-changed-a-field-at-a-time-and-a-break-can-take-tasks ()
  (lumenna-test--with-store
    (lumenna-write "block.add" :title "Train" :at "00:00" :minutes 1439 :date "today" :kind "break")
    (lumenna-today)
    (lumenna-test--goto "Train")
    (should-not (string-match-p "assigned" (lumenna-test--line)))
    (lumenna-test--answering ("Takes tasks" t)
      (lumenna-day-edit-block))
    (lumenna-test--goto "Train")
    (should (string-match-p "break block, now, takes tasks, nothing assigned" (lumenna-test--line)))
    (lumenna-test--answering ("Notes" "window seat")
      (lumenna-day-edit-block))
    (let ((id (plist-get (aref (plist-get (lumenna-call "block.list") :rows) 0) :id)))
      (should (equal (plist-get (lumenna-call "block.show" :id id) :notes) "window seat")))))

(ert-deftest lumenna-a-cancelled-day-is-listed-and-put-back ()
  (lumenna-test--with-store
    (lumenna-write "block.add" :title "Run" :at "7am" :minutes 30 :date "today" :repeat "every day")
    (lumenna-today)
    (lumenna-test--goto "Run")
    (lumenna-day-cancel)
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

;;;; Organising

(ert-deftest lumenna-a-project-is-made-renamed-archived-and-shows-its-tasks ()
  (lumenna-test--with-store
    (lumenna-projects)
    (lumenna-test--answering ("Wrok")
      (call-interactively #'lumenna-project-add))
    (lumenna-test--goto "Wrok")
    (lumenna-test--answering ("Work")
      (lumenna-project-rename))
    (lumenna-test--goto "Work")
    (lumenna-project-archive)
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
      (lumenna-label-merge))
    (should (equal (mapcar (lambda (r) (plist-get r :title))
                           (append (plist-get (lumenna-call "label.list") :rows) nil))
                   '("calls")))
    (lumenna-filters)
    (lumenna-test--answering ("Urgent" "p1")
      (lumenna-filter-add))
    (lumenna-test--goto "Urgent")
    (lumenna-test--answering ("p1 | p2")
      (lumenna-filter-requery))
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
      (lumenna-test--goto "  c: Complete, or mark not done")
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
  (should (memq 'lumenna-tasks-mode (function-get 'lumenna-task-toggle-done 'command-modes)))
  (should (memq 'lumenna-task-mode (function-get 'lumenna-task-toggle-done 'command-modes)))
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

(ert-deftest lumenna-a-running-daemon-is-used-over-its-socket ()
  (skip-when (eq system-type 'windows-nt))
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
