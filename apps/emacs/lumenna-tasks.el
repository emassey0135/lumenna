;;; lumenna-tasks.el --- Lumenna's task lists and task details -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Elijah Massey
;; SPDX-License-Identifier: MPL-2.0

;;; Commentary:

;; Task lists (any filter, any project or label), the trash, and one task's
;; details, where each field is changed in the minibuffer.  Every change goes
;; through the core and is undoable with u.

;;; Code:

(require 'lumenna)

(declare-function lumenna-assign-task "lumenna-day")

;;;; Lists

(defvar-local lumenna--prefix nil
  "What a task added in this list starts with: `#Work ' in a project's list.")

(define-derived-mode lumenna-tasks-mode lumenna-list-mode "Lumenna Tasks"
  "A list of tasks, subtasks folded under their task.
RET shows a task; ? shows every command.

\\{lumenna-tasks-mode-map}"
  (setq-local lumenna--activate #'lumenna-task-show))

(defun lumenna--task-listing (query title)
  "The heading and rows for QUERY, said back as it was understood (§6.2).
The heading starts with TITLE."
  (let* ((result (if (string-empty-p query)
                     (lumenna-call "task.list")
                   (lumenna-call "task.list" :query query)))
         (understood (plist-get (plist-get result :query) :description))
         (unresolved (mapcar (lambda (name)
                               (format "no %s called %s%s" (plist-get name :kind) (plist-get name :name)
                                       (if-let* ((near (plist-get name :suggestion)))
                                           (format ", did you mean %s?" near) "")))
                             (append (plist-get (plist-get result :query) :unresolved) nil))))
    (cons (string-join (delq nil (append (list (if understood (format "%s: %s" title understood) title)
                                               (plist-get result :announcement))
                                         unresolved))
                       ", ")
          (append (plist-get result :rows) nil))))

;;;###autoload
(defun lumenna-tasks (&optional query title prefix)
  "List the open tasks, or those matching QUERY, under TITLE.
PREFIX starts a task added here."
  (interactive)
  (let ((query (or query "")) (title (or title "Tasks")))
    (lumenna--show-list (format "*Lumenna: %s*" (if (string-empty-p query) title query))
                        #'lumenna-tasks-mode
                        (lambda () (lumenna--task-listing query title))
                        'lumenna--prefix prefix)))

(defun lumenna-tasks-add ()
  "Add a task, starting with this list's project or label if it has one."
  (interactive)
  (lumenna-add lumenna--prefix))

(defun lumenna--task-id ()
  "The task at point: in a list, or the one a detail or day buffer is about."
  (or (and (derived-mode-p 'lumenna-task-mode) lumenna--task)
      (let ((row (lumenna-row)))
        (or (plist-get row :task) (plist-get row :id)))))

(defun lumenna--task (&optional id)
  "Everything about task ID, or the one at point."
  (lumenna-call "task.show" :id (or id (lumenna--task-id))))

(defun lumenna-task-toggle-done ()
  "Complete the task at point, or mark a finished one not done."
  (interactive)
  (let* ((task (lumenna--task))
         (done (member "completed" (append (plist-get task :state) nil))))
    (lumenna-write (if done "task.undone" "task.done") :id (plist-get task :id))))

(defun lumenna-task-delete ()
  "Move the task at point to the trash; Lumenna's trash keeps it to restore."
  (interactive)
  (lumenna-write "task.rm" :id (lumenna--task-id))
  (when (derived-mode-p 'lumenna-task-mode) (quit-window)))

(defun lumenna--choose-task (prompt &optional excluding)
  "An open task chosen by title with completion, asking PROMPT.
Tasks whose ids are in EXCLUDING are not offered."
  (let* ((rows (seq-remove (lambda (row) (member (plist-get row :id) excluding))
                           (append (plist-get (lumenna-call "task.list") :rows) nil)))
         (choices (mapcar (lambda (row) (cons (lumenna-describe row) (plist-get row :id))) rows)))
    (unless choices (user-error "There are no other open tasks"))
    (cdr (assoc (completing-read prompt choices nil t) choices))))

(defun lumenna-task-make-subtask ()
  "Put the task at point under another, joining that one's project."
  (interactive)
  (let* ((id (lumenna--task-id))
         (parent (lumenna--choose-task "Make it a subtask of: " (list id))))
    (lumenna-write "task.move" :id id :parent parent)))

(defun lumenna-task-move-to-top ()
  "Take the task at point out from under its parent."
  (interactive)
  (lumenna-write "task.move" :id (lumenna--task-id) :top t))

(defun lumenna--project-names ()
  "Every project's name."
  (mapcar (lambda (row) (plist-get row :title)) (append (plist-get (lumenna-call "project.list") :rows) nil)))

(defun lumenna-task-move-to-project ()
  "Put the task at point in another project; its subtasks follow (§3.2)."
  (interactive)
  (let ((id (lumenna--task-id)))
    (lumenna-write "task.move" :id id
                   :project (completing-read "Move to project: " (lumenna--project-names) nil t))))

(defun lumenna-task-wait-for ()
  "Say the task at point cannot start until another is done (§3.3)."
  (interactive)
  (let* ((task (lumenna--task))
         (waiting (cons (plist-get task :id)
                        (mapcar (lambda (d) (plist-get d :id)) (append (plist-get task :depends) nil)))))
    (lumenna-write "task.depend.add" :id (plist-get task :id)
                   :on (lumenna--choose-task "Wait for: " waiting))))

(defun lumenna-task-stop-waiting ()
  "Stop the task at point waiting for one of the tasks it waits for."
  (interactive)
  (let* ((task (lumenna--task))
         (depends (mapcar (lambda (d) (cons (plist-get d :title) (plist-get d :id)))
                          (append (plist-get task :depends) nil))))
    (unless depends (user-error "It waits for nothing"))
    (lumenna-write "task.depend.rm" :id (plist-get task :id)
                   :on (cdr (assoc (completing-read "Stop waiting for: " depends nil t) depends)))))

(defun lumenna-task-assign ()
  "Put the task at point in a block, for a sitting (§3.7)."
  (interactive)
  (lumenna-assign-task (lumenna--task-id)))

;;;; Changing one field

(defconst lumenna--fields
  '(("Title" . title) ("Due" . due) ("Repeats" . repeat) ("Priority" . priority)
    ("Estimate" . estimate) ("Project" . project) ("Labels" . labels) ("Notes" . notes))
  "The fields of a task that can be changed, as they are named.")

(defun lumenna-task-edit (&optional field)
  "Change one FIELD of the task at point, asked for if not given."
  (interactive)
  (let* ((task (lumenna--task))
         (field (or field
                    (and (derived-mode-p 'lumenna-task-mode)
                         (get-text-property (line-beginning-position) 'lumenna-field))
                    (cdr (assoc (completing-read "Change: " lumenna--fields nil t) lumenna--fields)))))
    (if (eq field 'notes)
        (lumenna--edit-notes task)
      (let ((value (lumenna--read-field field task)))
        (lumenna-write "task.edit" :id (plist-get task :id) (intern (format ":%s" field)) value)))))

(defun lumenna--due-text (task)
  "TASK's due date as a phrase the core reads back: `2026-10-09 14:00'."
  (string-join (delq nil (list (plist-get task :due) (plist-get task :due_time))) " "))

(defun lumenna--read-field (field task)
  "Read a new value for FIELD of TASK, the way that field is best typed.
Dates and repetitions are phrases read by the core, as quick add reads them;
empty clears a field."
  (pcase field
    ('title (let ((title (read-string "Title: " (plist-get task :title))))
              (if (string-blank-p title) (user-error "A task needs a title") title)))
    ('due (let ((due (read-string "Due, such as tomorrow or next friday, empty for none: "
                                  (lumenna--due-text task))))
            (if (string-blank-p due) "none" due)))
    ('repeat (let ((repeat (read-string "Repeats, such as every monday, empty for none: "
                                        (or (plist-get task :repetition) ""))))
               (if (string-blank-p repeat) "none" repeat)))
    ('priority (let ((choices '(("1, highest" . 1) ("2" . 2) ("3" . 3) ("4, none" . 4))))
                 (cdr (assoc (completing-read "Priority: " choices nil t nil nil
                                              (car (rassoc (plist-get task :priority) choices)))
                             choices))))
    ('estimate (let ((estimate (read-string "Estimate, such as 45m or 1h30m, empty for none: "
                                            (if-let* ((minutes (plist-get task :estimate_mins)))
                                                (format "%sm" minutes) ""))))
                 (if (string-blank-p estimate) "none" estimate)))
    ('project (completing-read "Project: " (lumenna--project-names) nil t nil nil (plist-get task :project)))
    ('labels (vconcat
              (completing-read-multiple
               "Labels, separated by commas; a new name becomes a label: "
               (mapcar (lambda (row) (plist-get row :title))
                       (append (plist-get (lumenna-call "label.list") :rows) nil))
               nil nil (string-join (append (plist-get task :labels) nil) ","))))))

(defvar-local lumenna--notes-task nil "The task whose notes this buffer edits.")

(defvar-keymap lumenna-notes-mode-map
  :doc "Keys while editing a task's notes."
  "C-c C-c" #'lumenna-notes-save
  "C-c C-k" #'lumenna-notes-cancel)

(define-minor-mode lumenna-notes-mode
  "Editing a task's notes.
\\<lumenna-notes-mode-map>\\[lumenna-notes-save] saves them; \\[lumenna-notes-cancel] leaves them as they were."
  :lighter " Notes")

(defun lumenna--edit-notes (task)
  "Edit TASK's notes in a buffer of their own, as a commit message is edited."
  (let ((buffer (get-buffer-create (format "*Lumenna notes: %s*" (plist-get task :title)))))
    (pop-to-buffer buffer)
    (erase-buffer)
    (insert (or (plist-get task :notes) ""))
    (goto-char (point-min))
    (text-mode)
    (lumenna-notes-mode 1)
    (setq lumenna--notes-task (plist-get task :id))
    (message "Notes for %s.  C-c C-c saves, C-c C-k cancels" (plist-get task :title))))

(defun lumenna-notes-save ()
  "Save these notes to their task."
  (interactive)
  (let ((id lumenna--notes-task) (notes (buffer-substring-no-properties (point-min) (point-max))))
    (quit-window t)
    (lumenna-write "task.edit" :id id :notes notes)))

(defun lumenna-notes-cancel ()
  "Leave the notes as they were."
  (interactive)
  (quit-window t)
  (message "Notes unchanged"))

;;;; One task

(defvar-local lumenna--task nil "The task this detail buffer shows.")

(define-derived-mode lumenna-task-mode lumenna-list-mode "Lumenna Task"
  "One task, a field per line.  RET or e on a field changes it.

\\{lumenna-task-mode-map}"
  (setq-local lumenna--describe #'lumenna--describe-field)
  (setq-local lumenna--activate (lambda (row) (lumenna-task-edit (plist-get row :field)))))

(defun lumenna--describe-field (row)
  "A field ROW of a task's details, as its line."
  (format "%s: %s" (plist-get row :title) (plist-get row :value)))

(defun lumenna--task-fields (task)
  "TASK's details as rows, one field each: what can be changed, then what is known."
  (let* ((repeats (or (plist-get task :repetition)
                      (and (plist-get task :recurrence) (format "by the rule %s" (plist-get task :recurrence)))))
         (fields
          `(("Title" title ,(plist-get task :title))
            ("Due" due ,(let ((due (lumenna--due-text task))) (if (string-empty-p due) "none" due)))
            ("Repeats" repeat ,(or repeats "no"))
            ("Priority" priority ,(format "%s" (plist-get task :priority)))
            ("Estimate" estimate ,(if-let* ((m (plist-get task :estimate_mins))) (format "%s minutes" m) "none"))
            ("Project" project ,(or (plist-get task :project) "none"))
            ("Labels" labels ,(let ((labels (append (plist-get task :labels) nil)))
                                (if labels (string-join labels ", ") "none")))
            ("Waits for" nil ,(let ((d (append (plist-get task :depends) nil)))
                                (if d (string-join (mapcar (lambda (x) (plist-get x :title)) d) ", ") "nothing")))
            ("State" nil ,(string-join (append (plist-get task :state) nil) ", "))
            ("Notes" notes ,(let ((notes (plist-get task :notes)))
                              (if (string-empty-p notes) "none" notes))))))
    (mapcar (lambda (field)
              (list :key (car field) :title (car field) :field (nth 1 field) :value (nth 2 field)))
            fields)))

(defun lumenna-task-show (row)
  "Show the task in ROW, a field per line."
  (let ((id (or (plist-get row :task) (plist-get row :id))))
    (lumenna--show-list
     (format "*Lumenna task: %s*" (plist-get (lumenna--task id) :title))
     #'lumenna-task-mode
     (lambda ()
       (let ((task (lumenna--task id)))
         (cons (format "Task: %s" (plist-get task :title)) (lumenna--task-fields task))))
     'lumenna--task id)))

;; Field rows carry the field as a text property too, so e finds it.
(add-hook 'lumenna-row-inserted-functions
          (lambda (start end row)
            (when (plist-get row :field)
              (put-text-property start end 'lumenna-field (plist-get row :field)))))

;;;; The trash

(define-derived-mode lumenna-trash-mode lumenna-list-mode "Lumenna Trash"
  "Deleted tasks.  RET or r restores one; d erases it for good, asking first.

\\{lumenna-trash-mode-map}"
  (setq-local lumenna--activate (lambda (row) (lumenna-write "task.restore" :id (plist-get row :id)))))

;;;###autoload
(defun lumenna-trash ()
  "Show the trash: deleted tasks, to restore or erase (§3.2)."
  (interactive)
  (lumenna--show-list "*Lumenna: Trash*" #'lumenna-trash-mode
                      (lambda () (lumenna--task-listing "deleted" "Trash"))))

(defun lumenna-trash-restore ()
  "Take the task at point out of the trash."
  (interactive)
  (lumenna-write "task.restore" :id (plist-get (lumenna-row) :id)))

(defun lumenna-trash-erase ()
  "Erase the task at point and its history for good.  This cannot be undone (§9)."
  (interactive)
  (let ((row (lumenna-row)))
    (when (yes-or-no-p (format "%s and its history go for good, and this cannot be undone.  Erase it? "
                               (plist-get row :title)))
      (lumenna-write "task.erase" :id (plist-get row :id) :confirm t))))

;;;; Menus

(lumenna-define-keys lumenna-tasks-mode
  ("The task at point"
   ("RET" "Details" lumenna-activate)
   ("c" "Complete, or mark not done" lumenna-task-toggle-done)
   ("e" "Change a field" lumenna-task-edit)
   ("b" "Put it in a block" lumenna-task-assign)
   ("m" "Move to a project" lumenna-task-move-to-project)
   ("s" "Make it a subtask of another" lumenna-task-make-subtask)
   ("t" "Move it to the top level" lumenna-task-move-to-top)
   ("w" "Wait for another task" lumenna-task-wait-for)
   ("W" "Stop waiting for another" lumenna-task-stop-waiting)
   ("d" "Delete, to the trash" lumenna-task-delete)))

(lumenna-define-keys lumenna-task-mode
  ("This task"
   ("e" "Change the field at point, or choose one" lumenna-task-edit)
   ("c" "Complete, or mark not done" lumenna-task-toggle-done)
   ("b" "Put it in a block" lumenna-task-assign)
   ("m" "Move to a project" lumenna-task-move-to-project)
   ("s" "Make it a subtask of another" lumenna-task-make-subtask)
   ("t" "Move it to the top level" lumenna-task-move-to-top)
   ("w" "Wait for another task" lumenna-task-wait-for)
   ("W" "Stop waiting for another" lumenna-task-stop-waiting)
   ("d" "Delete, to the trash" lumenna-task-delete)))

(lumenna-define-keys lumenna-trash-mode
  ("The task at point"
   ("r" "Restore" lumenna-trash-restore)
   ("d" "Erase for good" lumenna-trash-erase)))

(provide 'lumenna-tasks)

;;; lumenna-tasks.el ends here
