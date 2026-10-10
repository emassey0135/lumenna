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
    (should-error (lumenna-act-rename) :type 'user-error)
    (let (offered)
      (cl-letf (((symbol-function 'completing-read)
                 (lambda (_prompt choices &rest _) (setq offered (mapcar #'car choices)) (car offered)))
                ((symbol-function 'read-string) (lambda (&rest _) "1")))
        (lumenna-act-at-point))
      (should (equal offered '("Weight"))))))

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
                ((symbol-function 'read-string) (lambda (&rest _) "30")))
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
  (let ((kill-ring nil) (kill-ring-yank-pointer nil) (interprogram-paste-function nil)
        (lumenna--shown-code nil))
    (kill-new "theirs123")
    (lumenna-test--answering ("")
      (should (equal (lumenna--read-code) "theirs123")))
    (setq lumenna--shown-code "theirs123")
    (lumenna-test--answering ("")
      (should-error (lumenna--read-code) :type 'user-error))
    (lumenna-test--answering ("typed456")
      (should (equal (lumenna--read-code) "typed456")))))

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
