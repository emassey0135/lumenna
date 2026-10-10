;;; lumenna-tasks.el --- Lumenna's task lists and task details -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Elijah Massey
;; SPDX-License-Identifier: MPL-2.0

;;; Commentary:

;; Task lists (any filter, any project or label), the trash, and one task's
;; details, where each field is changed in the minibuffer.  Every change goes
;; through the core and is undoable with u.

;;; Code:

(require 'lumenna)


;;;; Lists

(defvar-local lumenna--prefix nil
  "What a task added in this list starts with: `#Work ' in a project's list.")

(define-derived-mode lumenna-tasks-mode lumenna-list-mode "Lumenna Tasks"
  "A list of tasks, subtasks folded under their task.
RET shows a task; ? shows every command.

\\{lumenna-tasks-mode-map}"
  (setq-local lumenna--activate (lambda (row) (lumenna-task-show (plist-get row :id)))))

(defun lumenna--task-listing (query title)
  "The heading and rows for QUERY, said back as it was understood.
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
    (lumenna-listing (if understood (format "%s: %s" title understood) title) result nil unresolved)))

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

;; A task's actions are the row's own (`lumenna-act-kind'); these are the keys.
(lumenna-define-action lumenna-act-put-in-block ("put_in_block")
  "Put the task at point in a work block, for a sitting.")
(lumenna-define-action lumenna-act-move-to-project ("move_to_project")
  "Move the task at point to another project; its subtasks follow.")
(lumenna-define-action lumenna-act-make-subtask ("make_subtask_of")
  "Put the task at point under another.")
(lumenna-define-action lumenna-act-wait-for ("wait_for")
  "Say the task at point cannot start until another is done.")
(lumenna-define-action lumenna-act-stop-waiting ("stop_waiting")
  "Stop the task at point waiting for one it waits for.")

(defun lumenna--task (&optional id)
  "Everything about task ID, or the one this detail buffer shows."
  (lumenna-call "task.show" :id (or id lumenna--task)))

;;;; Changing one field

(defun lumenna--fields ()
  "The task form's fields, (LABEL . KEY) as the core names them, in its order."
  (mapcar (lambda (field) (cons (plist-get field :label) (intern (plist-get field :key))))
          (lumenna-form "task")))

(defun lumenna--field-label (key)
  "The task form's name for its field KEY, a symbol."
  (plist-get (lumenna-form-field "task" key) :label))

(defun lumenna--task-form (task)
  "TASK's fields as the core says a form starts from them (`form.task_fields')."
  (plist-get (lumenna-call "form.task_fields" :task task) :value))

(defun lumenna--save-task (task fields)
  "Save FIELDS over TASK, sending only what changed.
What changed is the core's to say (`form.task_edit')."
  (let ((edit (plist-get (lumenna-call "form.task_edit" :task task :fields fields) :value)))
    (if edit
        (apply #'lumenna-write "task.edit" :id (plist-get task :id) edit)
      (message "Nothing changed"))))

(defun lumenna-task-edit (&optional field)
  "Change one FIELD of this task, asked for if not given."
  (interactive)
  (let* ((task (lumenna--task))
         (field (or field
                    (get-text-property (line-beginning-position) 'lumenna-field)
                    (let ((fields (lumenna--fields)))
                      (cdr (assoc (completing-read "Change: " fields nil t) fields)))))
         (fields (lumenna--task-form task)))
    (if (eq field 'notes)
        (lumenna--edit-notes task fields)
      (let ((key (intern (format ":%s" field))))
        (lumenna--save-task task (plist-put (copy-sequence fields) key
                                            (lumenna--read-field field (plist-get fields key))))))))

(defun lumenna--read-field (field now)
  "Read a new value for FIELD, which is NOW, as the task form holds it.
Dates and repetitions are phrases read by the core, as quick add reads them;
empty clears a field.  Each is asked in the form's words: its hint, then its
name."
  (let* ((form (lumenna-form-field "task" field))
         (prompt (lumenna-field-prompt form)))
    (pcase field
      ('priority (let ((choices (mapcar (lambda (c) (cons (car c) (string-to-number (cdr c))))
                                        (lumenna-field-options form))))
                   (cdr (assoc (completing-read (lumenna--with-default prompt (car (rassoc now choices)))
                                                choices nil t nil nil (car (rassoc now choices)))
                               choices))))
      ('project (completing-read (lumenna--with-default prompt now) (lumenna--project-names) nil t nil nil now))
      ('labels (string-join
                (completing-read-multiple
                 prompt
                 (mapcar (lambda (row) (plist-get row :title))
                         (append (plist-get (lumenna-call "label.list") :rows) nil))
                 nil nil now)
                ", "))
      (_ (lumenna-read-field form prompt now)))))

(defun lumenna--project-names ()
  "Every project's name."
  (mapcar (lambda (row) (plist-get row :title)) (append (plist-get (lumenna-call "project.list") :rows) nil)))

(defvar-local lumenna--notes-task nil "The task whose notes this buffer edits, as shown.")
(defvar-local lumenna--notes-fields nil "The task's form fields, as they were when editing began.")

(defvar-keymap lumenna-notes-mode-map
  :doc "Keys while editing a task's notes.
C-c and a control character are a major mode's to bind, so editing notes is
a major mode of its own, as `log-edit-mode' is for a commit message."
  "C-c C-c" #'lumenna-notes-save
  "C-c C-k" #'lumenna-notes-cancel)

(define-derived-mode lumenna-notes-mode text-mode "Lumenna Notes"
  "Editing a task's notes.
\\<lumenna-notes-mode-map>\\[lumenna-notes-save] saves them; \\[lumenna-notes-cancel] leaves them as they were.")

(defun lumenna--edit-notes (task fields)
  "Edit TASK's notes, from its form FIELDS, in a buffer of their own.
As a commit message is edited."
  (let ((buffer (get-buffer-create (format "*Lumenna notes: %s*" (plist-get task :title)))))
    (pop-to-buffer buffer)
    (erase-buffer)
    (insert (or (plist-get task :notes) ""))
    (goto-char (point-min))
    (lumenna-notes-mode)
    (setq lumenna--notes-task task lumenna--notes-fields fields)
    (message "Notes for %s.  C-c C-c saves, C-c C-k cancels" (plist-get task :title))))

(defun lumenna-notes-save ()
  "Save these notes to their task."
  (interactive)
  (let ((task lumenna--notes-task)
        (fields (plist-put (copy-sequence lumenna--notes-fields) :notes
                           (buffer-substring-no-properties (point-min) (point-max)))))
    (quit-window t)
    (lumenna--save-task task fields)))

(defun lumenna-notes-cancel ()
  "Leave the notes as they were."
  (interactive)
  (quit-window t)
  (message "Notes unchanged"))

;;;; One task

(defvar-local lumenna--task nil "The task this detail buffer shows.")

(define-derived-mode lumenna-task-mode lumenna-list-mode "Lumenna Task"
  "One task, a field per line: its form.  RET or e on a field changes it.
The other keys are the task's own actions, wherever point is.

\\{lumenna-task-mode-map}"
  (setq-local lumenna--describe #'lumenna--describe-field)
  (setq-local lumenna--actions-function (lambda () (plist-get (lumenna--task) :actions)))
  (setq-local lumenna--activate (lambda (row) (lumenna-task-edit (plist-get row :field)))))

(defun lumenna--describe-field (row)
  "A field ROW of a task's details, as its line."
  (format "%s: %s" (plist-get row :title) (plist-get row :value)))

(defun lumenna--task-fields (task)
  "TASK's details as rows, one field each: its form, then what is known."
  (let* ((form (lumenna--task-form task))
         (shown (lambda (key) (let ((value (plist-get form key)))
                                (if (string-empty-p value) "none" value))))
         (label #'lumenna--field-label)
         (fields
          `((,(funcall label 'title) title ,(plist-get form :title))
            (,(funcall label 'due) due ,(funcall shown :due))
            ;; A rule the words cannot say leaves the field empty; say the rule.
            (,(funcall label 'repeat) repeat ,(cond ((not (string-empty-p (plist-get form :repeat))) (plist-get form :repeat))
                                     ((plist-get task :recurrence) (format "by the rule %s" (plist-get task :recurrence)))
                                     (t "no")))
            (,(funcall label 'priority) priority
             ,(let ((id (format "%s" (plist-get form :priority))))
                (or (car (rassoc id (lumenna-field-options (lumenna-form-field "task" 'priority)))) id)))
            (,(funcall label 'estimate) estimate ,(funcall shown :estimate))
            (,(funcall label 'project) project ,(funcall shown :project))
            (,(funcall label 'labels) labels ,(funcall shown :labels))
            ("Waits for" nil ,(let ((d (append (plist-get task :depends) nil)))
                                (if d (string-join (mapcar (lambda (x) (plist-get x :title)) d) ", ") "nothing")))
            ("State" nil ,(string-join (append (plist-get task :state) nil) ", "))
            (,(funcall label 'notes) notes ,(funcall shown :notes)))))
    (mapcar (lambda (field)
              (list :key (car field) :title (car field) :field (nth 1 field) :value (nth 2 field)))
            fields)))

(defun lumenna-task-show (id)
  "Show task ID, a field per line."
  (lumenna--show-list
   (format "*Lumenna task: %s*" (plist-get (lumenna--task id) :title))
   #'lumenna-task-mode
   (lambda ()
     (let ((task (lumenna--task id)))
       (cons (format "Task: %s" (plist-get task :title)) (lumenna--task-fields task))))
   'lumenna--task id))

;; The task form, wherever an action asks for it: a task's Edit Details, a
;; sitting's Edit Task Details.
(lumenna-define-form "task" "edit" (lambda (action _row) (lumenna-task-show (plist-get action :target))))
(lumenna-define-form "task" "edit_task" (lambda (action _row) (lumenna-task-show (plist-get action :target))))

;; A task moved to the trash from its own buffer leaves it.
(add-hook 'lumenna-changed-functions
          (lambda (event _result)
            (when (and (equal event "task/delete") (derived-mode-p 'lumenna-task-mode))
              (quit-window))))

;; Field rows carry the field as a text property too, so e finds it.
(add-hook 'lumenna-row-inserted-functions
          (lambda (start end row)
            (when (plist-get row :field)
              (put-text-property start end 'lumenna-field (plist-get row :field)))))

;;;; The trash

(define-derived-mode lumenna-trash-mode lumenna-list-mode "Lumenna Trash"
  "Deleted tasks.
RET or r restores one; d deletes it from the trash, asking first.

\\{lumenna-trash-mode-map}"
  (setq-local lumenna--activate (lambda (_row) (lumenna-act-restore))))

;;;###autoload
(defun lumenna-trash ()
  "Show the trash: deleted tasks, to restore or delete."
  (interactive)
  (lumenna--show-list "*Lumenna: Trash*" #'lumenna-trash-mode
                      (lambda () (lumenna--task-listing "deleted" "Trash"))))

;;;; Menus

(lumenna-define-keys lumenna-tasks-mode
  ("The task at point"
   ("RET" "Details" lumenna-activate)
   ("c" "Mark Done, or Mark Not Done" lumenna-act-done)
   ("e" "Edit Details" lumenna-act-edit)
   ("b" "Put in a Block" lumenna-act-put-in-block)
   ("m" "Move to Project" lumenna-act-move-to-project)
   ("s" "Make Subtask Of" lumenna-act-make-subtask)
   ("t" "Move to Top Level" lumenna-act-move-to-top)
   ("w" "Wait For" lumenna-act-wait-for)
   ("W" "Stop Waiting" lumenna-act-stop-waiting)
   ("d" "Move to Trash" lumenna-act-delete)))

(lumenna-define-keys lumenna-task-mode
  ("This task"
   ("e" "Change the field at point, or choose one" lumenna-task-edit)
   ("c" "Mark Done, or Mark Not Done" lumenna-act-done)
   ("b" "Put in a Block" lumenna-act-put-in-block)
   ("m" "Move to Project" lumenna-act-move-to-project)
   ("s" "Make Subtask Of" lumenna-act-make-subtask)
   ("t" "Move to Top Level" lumenna-act-move-to-top)
   ("w" "Wait For" lumenna-act-wait-for)
   ("W" "Stop Waiting" lumenna-act-stop-waiting)
   ("d" "Move to Trash" lumenna-act-delete)))

(lumenna-define-keys lumenna-trash-mode
  ("The task at point"
   ("r" "Restore" lumenna-act-restore)
   ("d" "Delete from Trash" lumenna-act-delete)))

(provide 'lumenna-tasks)

;;; lumenna-tasks.el ends here
