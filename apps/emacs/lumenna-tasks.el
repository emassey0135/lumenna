;;; lumenna-tasks.el --- Lumenna's task lists and task details -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Elijah Massey
;; SPDX-License-Identifier: MPL-2.0

;;; Commentary:

;; Task lists (every task, a project's, a label's, a saved filter's, a search,
;; the trash), each a screen of the app's, and one task's details, a screen of
;; its own, where each field is changed in the minibuffer.  Every change goes
;; through the app and is undoable with u.

;;; Code:

(require 'lumenna)

;;;; Lists

(defun lumenna--list-rows ()
  "The heading and rows of the list this buffer's screen shows.
The heading is the place's name, then how the list reads back: the query
understood, its count, what it could not find."
  (let* ((list (plist-get lumenna--view :list))
         (items (append (plist-get list :items) nil))
         (lines (append (plist-get lumenna--lines :list) nil))
         (rows (delq nil (cl-mapcar (lambda (item line)
                                      (unless (lumenna--true (plist-get item :hidden))
                                        (append (list :key (plist-get (plist-get item :row) :id)
                                                      :text (plist-get line :text))
                                                (plist-get item :row))))
                                    items lines))))
    (setq-local lumenna--empty (plist-get list :empty))
    (cons (lumenna--heading (plist-get lumenna--view :title)
                            ;; An empty list says so in the line under its heading.
                            (and rows (plist-get lumenna--lines :readback))
                            (and (null rows) (plist-get list :refusal)))
          rows)))

(define-derived-mode lumenna-tasks-mode lumenna-list-mode "Lumenna Tasks"
  "A list of tasks, subtasks folded under their task.
RET shows a task; ? shows every command.

\\{lumenna-tasks-mode-map}"
  (setq-local lumenna--draw #'lumenna--list-rows)
  (setq-local lumenna--activate (lambda (row) (lumenna-task-show (plist-get row :id)))))

(defun lumenna--place-title (place)
  "What PLACE, as the app names it, is called in its buffer's name."
  (pcase place
    ("Tasks" "Tasks") ("Trash" "Trash") ("Blocks" "Blocks") ("Today" "Day")
    ((pred (lambda (p) (plist-get p :Project))) (plist-get place :Project))
    ((pred (lambda (p) (plist-get p :Label))) (plist-get place :Label))
    ((pred (lambda (p) (plist-get p :Filter))) (plist-get (plist-get place :Filter) :name))
    (_ (format "%s" place))))

(defvar-local lumenna--place nil
  "The place this buffer was opened on, as the app names it.")

(declare-function lumenna-today "lumenna-day" ())
(declare-function lumenna-blocks-mode "lumenna-day" ())

(defun lumenna-open-place (place)
  "Open PLACE, as the app names it, in a buffer of its own: its screen's.
A buffer that shows it already is shown again."
  (if (equal place "Today")
      (lumenna-today)
    (let ((mode (pcase place ("Trash" #'lumenna-trash-mode) ("Blocks" #'lumenna-blocks-mode) (_ #'lumenna-tasks-mode)))
          (name (format "*Lumenna: %s*" (lumenna--place-title place))))
      (lumenna--show-screen name mode (list (list :Open place)))
      (setq lumenna--place place))))

;;;###autoload
(defun lumenna-tasks ()
  "List the open tasks."
  (interactive)
  (lumenna-open-place "Tasks"))

;;;###autoload
(defun lumenna-search (query)
  "List the tasks matching QUERY, a filter such as \"#Work & overdue\".
\"search:\" followed by words looks through titles and notes.  TAB completes."
  (interactive (list (lumenna-read-line "Filter, or search: and words: " "filter" nil 'lumenna-filter-history)))
  (lumenna--show-screen (format "*Lumenna: %s*" query) #'lumenna-tasks-mode
                        (list (list :Open "Tasks") (list :Filter query))))

(defun lumenna-tasks-filter (query)
  "Filter this list by QUERY, in place of its own; empty for its own again."
  (interactive (list (lumenna-read-line "Filter, or search: and words: " "filter"
                                        (let ((typed (plist-get lumenna--view :query))) (and typed (not (string-empty-p typed)) typed))
                                        'lumenna-filter-history)))
  (lumenna-send (list :Filter query))
  (lumenna--follow))

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

;;;; One task

(defvar-local lumenna--task nil "The task this detail buffer shows.")

(define-derived-mode lumenna-task-mode lumenna-list-mode "Lumenna Task"
  "One task, a field per line: its form.  RET or e on a field changes it.
The other keys are the task's own actions, wherever point is.

\\{lumenna-task-mode-map}"
  (setq-local lumenna--draw #'lumenna--task-rows)
  (setq-local lumenna--actions-function (lambda () (plist-get (lumenna--details-task) :actions)))
  (setq-local lumenna--activate (lambda (row)
                                  (if (plist-get row :line)
                                      (message "%s" (plist-get row :text))
                                    (lumenna-task-edit (plist-get row :field))))))

(defun lumenna--details ()
  "The details this buffer's screen shows."
  (or (plist-get lumenna--view :details) (user-error "No task here")))

(defun lumenna--details-task ()
  "The task this buffer's screen shows, as the store has it."
  (plist-get (plist-get lumenna--view :details) :task))

(defun lumenna--form-field (key)
  "The task form's field KEY, a string, as the app gives it."
  (seq-find (lambda (field) (equal (plist-get field :key) key))
            (append (plist-get (lumenna--details) :form) nil)))

(defun lumenna--field-value (field fields task)
  "What FIELD of the form holds in FIELDS, as a line says it.
TASK gives what the words cannot say: a rule that repeats."
  (let* ((key (plist-get field :key))
         (value (plist-get fields (intern (concat ":" key)))))
    (pcase key
      ("priority" (let ((id (format "%s" value)))
                    (or (plist-get (seq-find (lambda (o) (equal (plist-get o :id) id))
                                             (append (plist-get field :options) nil))
                                   :title)
                        id)))
      ;; A rule the words cannot say leaves the field empty; say the rule.
      ("repeat" (cond ((not (string-empty-p value)) value)
                      ((plist-get task :recurrence) (format "by the rule %s" (plist-get task :recurrence)))
                      (t "no")))
      (_ (if (or (null value) (and (stringp value) (string-empty-p value))) "none" (format "%s" value))))))

(defun lumenna--task-rows ()
  "The task's details as rows: each field of its form, what it waits for, its
state, then its history under a heading of its own."
  (let* ((details (plist-get lumenna--view :details))
         (task (plist-get details :task))
         (fields (plist-get details :fields))
         (form (append (plist-get details :form) nil))
         (row (lambda (title field value)
                (list :key title :title title :field field :value value :text (format "%s: %s" title value))))
         (shown (mapcar (lambda (field)
                          (funcall row (plist-get field :label) (plist-get field :key)
                                   (lumenna--field-value field fields task)))
                        (seq-remove (lambda (f) (equal (plist-get f :key) "notes")) form)))
         (notes (seq-find (lambda (f) (equal (plist-get f :key) "notes")) form))
         (history (append (plist-get task :history) nil)))
    (setq-local lumenna--empty nil)
    (if (null details)
        (list "No task")
      (cons (format "Task: %s" (plist-get task :title))
            (append shown
                    (list (funcall row "Waits for" nil
                                   (let ((d (append (plist-get task :depends) nil)))
                                     (if d (string-join (mapcar (lambda (x) (plist-get x :title)) d) ", ") "nothing")))
                          (funcall row "State" nil (plist-get lumenna--lines :task_state)))
                    (and notes (list (funcall row (plist-get notes :label) "notes"
                                              (lumenna--field-value notes fields task))))
                    ;; Asked for about one task, on its own screen, never in a list.
                    (and history
                         (cons (list :key "History" :title "History" :text "History" :line t
                                     :role "heading" :face 'lumenna-heading)
                               (mapcar (lambda (line) (list :key (concat "History: " line) :text line :line t :depth 1))
                                       history))))))))

(defun lumenna-task-show (id)
  "Show task ID, a field per line: a screen of its own, holding the task."
  (set-buffer (lumenna--show-screen (format "*Lumenna task: %s*" id) #'lumenna-task-mode
                                    (list (list :Details (list :Show id)))))
  (setq lumenna--task id)
  (let ((title (plist-get (lumenna--details-task) :title)))
    (when title (rename-buffer (format "*Lumenna task: %s*" title) t))))

;; The task form, wherever an action asks for it: a task's Edit Details, a
;; sitting's Edit Task Details.
(lumenna-define-form "task" "edit" (lambda (action) (lumenna-task-show (plist-get action :target))))
(lumenna-define-form "task" "edit_task" (lambda (action) (lumenna-task-show (plist-get action :target))))

;;;; Changing one field

(defun lumenna--save-field (key value)
  "Set the field KEY, a string, to VALUE, and save what changed: the app's way."
  (lumenna-send (list :Details (list :Field (list :key key :value value))))
  (lumenna-send (list :Details "Save"))
  (lumenna--follow))

(defun lumenna-task-edit (&optional key)
  "Change the field KEY of this task, a string, asked for if not given."
  (interactive)
  (let* ((form (append (plist-get (lumenna--details) :form) nil))
         (key (or key
                  (get-text-property (line-beginning-position) 'lumenna-field)
                  (let ((fields (mapcar (lambda (f) (cons (plist-get f :label) (plist-get f :key))) form)))
                    (cdr (assoc (completing-read "Change: " fields nil t) fields)))
                  (user-error "No field here")))
         (field (lumenna--form-field key))
         (fields (plist-get (lumenna--details) :fields))
         (now (plist-get fields (intern (concat ":" key)))))
    (if (equal key "notes")
        (lumenna--edit-notes (lumenna--details-task) now)
      (lumenna--save-field key (lumenna--read-task-field field now)))))

(defun lumenna--read-task-field (field now)
  "Read a new value for FIELD of the task form, which is NOW.
Dates and repetitions are phrases read by the core, as quick add reads them;
empty clears a field.  Each is asked in the form's words: its hint, then its
name."
  (let ((prompt (lumenna--prompt (plist-get field :hint) (plist-get field :label)))
        (example (plist-get field :example)))
    (pcase (plist-get field :key)
      ("priority"
       (let* ((choices (mapcar (lambda (o) (cons (plist-get o :title) (plist-get o :id)))
                               (append (plist-get field :options) nil)))
              (default (car (rassoc (format "%s" now) choices))))
         (cdr (assoc (completing-read (lumenna--with-default prompt default) choices nil t nil nil default)
                     choices))))
      ("project" (lumenna--read-project prompt now))
      ("labels" (string-join (completing-read-multiple prompt (lumenna--label-names) nil nil now) ", "))
      (_ (read-from-minibuffer prompt now nil nil nil (and example (not (string-empty-p example)) example))))))

(defun lumenna--label-names ()
  "Every label's name, from the places."
  (with-current-buffer (lumenna--home-buffer)
    (delq nil (mapcar (lambda (entry) (and (plist-get (plist-get entry :kind) :Place)
                                           (plist-get (plist-get (plist-get entry :kind) :Place) :Label)
                                           (plist-get entry :title)))
                      (append (plist-get lumenna--view :sidebar) nil)))))

(defun lumenna--read-project (prompt now)
  "Read a project, asking PROMPT: one of those the details offer.
NOW, the task's project, is the default.  Only an offered project is taken,
and typing finds one.  A project's level in the tree is its annotation, as
an outline says it, never indentation alone."
  (let* ((options (append (plist-get (lumenna--details) :projects) nil))
         ;; An archived project is offered to no task, but the one this task is in stays
         ;; on its list, or the field would lose it.
         (options (if (or (null now) (string-empty-p now) (seq-find (lambda (o) (equal (plist-get o :id) now)) options))
                      options
                    (append options (list (list :id now :title now :depth 0)))))
         (lines (lumenna--unique (mapcar (lambda (o) (cons (plist-get o :title) o)) options)))
         (completion-extra-properties
          (list :annotation-function
                (lambda (line)
                  (let ((depth (or (plist-get (cdr (assoc line lines)) :depth) 0)))
                    (if (> depth 0) (format "  level %d" (1+ depth)) "")))))
         (default (car (seq-find (lambda (l) (equal (plist-get (cdr l) :id) now)) lines)))
         (picked (completing-read (lumenna--with-default prompt default) lines nil t nil nil default)))
    (if-let* ((option (cdr (assoc picked lines))))
        (plist-get option :id)
      now)))

(defvar-local lumenna--notes-buffer nil "The task buffer whose notes this edits.")

(defvar-keymap lumenna-notes-mode-map
  :doc "Keys while editing a task's notes.
C-c and a control character are a major mode's to bind, so editing notes is
a major mode of its own, as `log-edit-mode' is for a commit message."
  "C-c C-c" #'lumenna-notes-save
  "C-c C-k" #'lumenna-notes-cancel)

(define-derived-mode lumenna-notes-mode text-mode "Lumenna Notes"
  "Editing a task's notes.
\\<lumenna-notes-mode-map>\\[lumenna-notes-save] saves them; \\[lumenna-notes-cancel] leaves them as they were.")

(defun lumenna--edit-notes (task notes)
  "Edit TASK's NOTES in a buffer of their own, as a commit message is edited."
  (let ((origin (current-buffer))
        (buffer (get-buffer-create (format "*Lumenna notes: %s*" (plist-get task :title)))))
    (pop-to-buffer buffer)
    (erase-buffer)
    (insert (or notes ""))
    (goto-char (point-min))
    (lumenna-notes-mode)
    (setq lumenna--notes-buffer origin)
    (message "Notes for %s.  C-c C-c saves, C-c C-k cancels" (plist-get task :title))))

(defun lumenna-notes-save ()
  "Save these notes to their task."
  (interactive)
  (let ((origin lumenna--notes-buffer)
        (notes (buffer-substring-no-properties (point-min) (point-max))))
    (quit-window t)
    (with-current-buffer origin (lumenna--save-field "notes" notes))))

(defun lumenna-notes-cancel ()
  "Leave the notes as they were."
  (interactive)
  (quit-window t)
  (message "Notes unchanged"))

(defun lumenna-task-delete ()
  "Move this task to the trash, as it offers, and leave its buffer."
  (interactive)
  (lumenna-act-kind '("delete"))
  ;; Gone to the trash, it no longer offers Move to Trash.
  (unless (seq-find (lambda (a) (equal (plist-get a :kind) "delete")) (lumenna-actions))
    (quit-window)))

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
  (setq-local lumenna--draw #'lumenna--list-rows)
  (setq-local lumenna--activate (lambda (_row) (lumenna-act-restore))))

;;;###autoload
(defun lumenna-trash ()
  "Show the trash: deleted tasks, to restore or delete."
  (interactive)
  (lumenna-open-place "Trash"))

;;;###autoload
(defun lumenna-history ()
  "Show what was done over the last two weeks, newest first."
  (interactive)
  (lumenna-open-place "History"))

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
   ("R" "Reminders" lumenna-act-reminders)
   ("d" "Move to Trash" lumenna-act-delete))
  ("This list"
   ("a" "Add a task" lumenna-add)
   ("/" "Filter this list" lumenna-tasks-filter)))

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
   ("R" "Reminders" lumenna-act-reminders)
   ("d" "Move to Trash" lumenna-task-delete)))

(lumenna-define-keys lumenna-trash-mode
  ("The task at point"
   ("r" "Restore" lumenna-act-restore)
   ("d" "Delete from Trash" lumenna-act-delete)))

(provide 'lumenna-tasks)

;;; lumenna-tasks.el ends here
