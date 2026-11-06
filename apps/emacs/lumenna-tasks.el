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
         (offered (plist-get list :offered))
         (rows (delq nil (cl-mapcar (lambda (item line)
                                      (unless (lumenna--true (plist-get item :hidden))
                                        ;; A list's rows name their actions in short, each
                                        ;; held once in the list: put in place of the row's.
                                        (append (list :key (plist-get (plist-get item :row) :id)
                                                      :text (plist-get line :text)
                                                      :actions (mapcar (lambda (at) (aref offered at))
                                                                       (plist-get item :actions)))
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
  (lumenna--mode-name '(command "go-tasks"))
  (setq-local lumenna--draw #'lumenna--list-rows)
  (setq-local lumenna--activate (lambda (row) (lumenna-task-show (plist-get row :id)))))

(defun lumenna--place-title (place)
  "What PLACE, as the app names it, is called in its buffer's name: the command
that goes there names it, and the day's buffer is its group's."
  (pcase place
    ("Tasks" (lumenna-word '(command "go-tasks"))) ("Trash" (lumenna-word '(command "go-trash")))
    ("Blocks" (lumenna-word '(command "go-blocks"))) ("History" (lumenna-word '(command "go-history")))
    ("Today" (lumenna-word '(group "day")))
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
          (name (lumenna--buffer-name (lumenna--place-title place))))
      (lumenna--show-screen name mode (list (list :Open place)))
      (setq lumenna--place place))))

;;;###autoload
(defun lumenna-tasks ()
  "List the open tasks."
  (interactive)
  (lumenna-open-place "Tasks"))

;;;###autoload
(defun lumenna--filter-prompt ()
  "What a filter is asked with: the filter's hint, then its name."
  (lumenna--prompt (lumenna-words :quick_add :filter_hint) (lumenna-words :quick_add :filter)))

(defun lumenna-search (query)
  "List the tasks matching QUERY, a filter such as \"#Work & overdue\".
\"search:\" followed by words looks through titles and notes.  TAB completes."
  (interactive (list (lumenna-read-line (lumenna--filter-prompt) "filter" nil 'lumenna-filter-history)))
  (lumenna--show-screen (lumenna--buffer-name query) #'lumenna-tasks-mode
                        (list (list :Open "Tasks") (list :Filter query))))

(defun lumenna-tasks-filter (query)
  "Filter this list by QUERY, in place of its own; empty for its own again."
  (interactive (list (lumenna-read-line (lumenna--filter-prompt) "filter"
                                        (let ((typed (plist-get lumenna--view :query))) (and typed (not (string-empty-p typed)) typed))
                                        'lumenna-filter-history)))
  (lumenna-send (list :Filter query))
  (lumenna--follow))

;; A task's commands are the shared table's, which the core runs on the row
;; (`lumenna-press'); these are the keys.
(lumenna-define-command lumenna-act-put-in-block "put-in-block"
  "Put the task at point in a work block, for a sitting.")
(lumenna-define-command lumenna-act-move-to-project "move-to-project"
  "Move the task at point to another project; its subtasks follow.")
(lumenna-define-command lumenna-act-make-subtask "make-subtask"
  "Put the task at point under another.")
(lumenna-define-command lumenna-act-wait-for "wait-for"
  "Say the task at point cannot start until another is done.")
;; One Stop Waiting per task waited for: the row's own, so the person chooses.
(lumenna-define-action lumenna-act-stop-waiting ("stop_waiting")
  "Stop the task at point waiting for one it waits for.")

;;;; One task

(defvar-local lumenna--task nil "The task this detail buffer shows.")

(define-derived-mode lumenna-task-mode lumenna-list-mode "Lumenna Task"
  "One task, a field per line: its form.  RET or e on a field changes it.
The other keys are the task's own actions, wherever point is.

\\{lumenna-task-mode-map}"
  (lumenna--mode-name '(group "task"))
  (setq-local lumenna--draw #'lumenna--task-rows)
  (setq-local lumenna--actions-function (lambda () (plist-get (lumenna--details-task) :offered)))
  (setq-local lumenna--activate (lambda (row)
                                  (if (plist-get row :line)
                                      (message "%s" (plist-get row :text))
                                    (lumenna-task-edit (plist-get row :field))))))

(defun lumenna--details ()
  "The details this buffer's screen shows."
  (or (plist-get lumenna--view :details) (lumenna--refusal :no_task)))

(defun lumenna--details-task ()
  "The task this buffer's screen shows, as the store has it."
  (plist-get (plist-get lumenna--view :details) :task))

(defun lumenna--form-field (key)
  "The task form's field KEY, a string, as the app gives it."
  (seq-find (lambda (field) (equal (plist-get field :key) key))
            (append (plist-get (lumenna--details) :form) nil)))

(defun lumenna--field-value (field fields)
  "What FIELD of the form holds in FIELDS, as a line says it.
A priority is its option's words; an empty field says so."
  (let* ((key (plist-get field :key))
         (value (plist-get fields (intern (concat ":" key)))))
    (pcase key
      ("priority" (let ((id (format "%s" value)))
                    (or (plist-get (seq-find (lambda (o) (equal (plist-get o :id) id))
                                             (append (plist-get field :options) nil))
                                   :title)
                        id)))
      ;; An empty field is said as the details' own lines say one.
      (_ (if (or (null value) (and (stringp value) (string-empty-p value)))
             (lumenna-words :none_value)
           (format "%s" value))))))

(defun lumenna--task-rows ()
  "The task's details as rows: each field of its form, then what the details
list besides it in the core's words (its state, the rule it repeats by, what
it waits for), its notes, and its history last."
  (let* ((details (plist-get lumenna--view :details))
         (task (plist-get details :task))
         (fields (plist-get details :fields))
         (form (append (plist-get details :form) nil))
         (row (lambda (title field value)
                (list :key title :title title :field field :value value :text (format "%s: %s" title value))))
         (shown (mapcar (lambda (field)
                          (funcall row (plist-get field :label) (plist-get field :key)
                                   (lumenna--field-value field fields)))
                        (seq-remove (lambda (f) (equal (plist-get f :key) "notes")) form)))
         (notes (seq-find (lambda (f) (equal (plist-get f :key) "notes")) form))
         ;; Each line the details list, worded by the app; asked for about one task, on its
         ;; own screen, its history is never in a list.
         (said (cl-mapcar (lambda (line words)
                            (list :key (concat "about " (plist-get line :key)) :about (plist-get line :key)
                                  :text (plist-get words :text) :line t))
                          (append (plist-get details :lines) nil)
                          (append (plist-get lumenna--lines :details) nil)))
         (history (seq-filter (lambda (r) (equal (plist-get r :about) "history")) said)))
    (setq-local lumenna--empty nil)
    (if (null details)
        (list (lumenna-words :details :gone))
      (cons (format "%s: %s" (lumenna-word '(group "task")) (plist-get task :title))
            (append shown
                    (seq-difference said history)
                    (and notes (list (funcall row (plist-get notes :label) "notes"
                                              (lumenna--field-value notes fields))))
                    history)))))

(defun lumenna-task-show (id)
  "Show task ID, a field per line: a screen of its own, holding the task."
  (set-buffer (lumenna--show-screen (lumenna--buffer-name (lumenna-word '(group "task")) id) #'lumenna-task-mode
                                    (list (list :Details (list :Show id)))))
  (setq lumenna--task id)
  (let ((title (plist-get (lumenna--details-task) :title)))
    (when title (rename-buffer (lumenna--buffer-name (lumenna-word '(group "task")) title) t))))

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
                    (cdr (assoc (completing-read (lumenna--prompt (lumenna-word :change)) fields nil t) fields)))
                  (lumenna--refusal :no_field)))
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
and typing finds one.  A project's level in the tree is its annotation, the
app's words where it changes, as an outline says it, never indentation alone."
  (let* ((offered (append (plist-get (lumenna--details) :projects) nil))
         (levels (mapcar (lambda (line) (plist-get line :level)) (append (plist-get lumenna--lines :projects) nil)))
         (options (cl-mapcar (lambda (option index) (append option (list :level (nth index levels))))
                             offered (number-sequence 0 (1- (length offered)))))
         ;; An archived project is offered to no task, but the one this task is in stays
         ;; on its list, or the field would lose it.
         (options (if (or (null now) (string-empty-p now) (seq-find (lambda (o) (equal (plist-get o :id) now)) options))
                      options
                    (append options (list (list :id now :title now :depth 0)))))
         (lines (lumenna--unique (mapcar (lambda (o) (cons (plist-get o :title) o)) options)))
         (completion-extra-properties
          (list :annotation-function
                (lambda (line)
                  (let ((level (plist-get (cdr (assoc line lines)) :level)))
                    (if (and level (not (string-empty-p level))) (concat "  " level) "")))))
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
\\<lumenna-notes-mode-map>\\[lumenna-notes-save] saves them; \\[lumenna-notes-cancel] leaves them as they were."
  (lumenna--mode-name :notes))

(defun lumenna--edit-notes (task notes)
  "Edit TASK's NOTES in a buffer of their own, as a commit message is edited."
  (let ((origin (current-buffer))
        (buffer (get-buffer-create (lumenna--buffer-name (lumenna-word :notes) (plist-get task :title)))))
    (pop-to-buffer buffer)
    (erase-buffer)
    (insert (or notes ""))
    (goto-char (point-min))
    (lumenna-notes-mode)
    (setq lumenna--notes-buffer origin)
    (message "%s" (format (lumenna-word :notes_for) (plist-get task :title) "C-c C-c" "C-c C-k"))))

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
  (message "%s" (lumenna-word :notes_unchanged)))

(defun lumenna-task-delete ()
  "Delete this task as the delete key does, and leave its buffer.
To the trash, or from the trash for one there already: the core's."
  (interactive)
  (let ((before (lumenna-actions)))
    (lumenna-press "delete")
    ;; Moved to the trash or out of it, what the task offers changed; refused or
    ;; let go, it is as it was.
    (unless (and (lumenna--details-task) (equal before (lumenna-actions)))
      (quit-window))))

;; Field rows carry the field as a text property too, so e finds it.
(add-hook 'lumenna-row-inserted-functions
          (lambda (start end row)
            (when (plist-get row :field)
              (put-text-property start end 'lumenna-field (plist-get row :field)))))

;;;; The trash

(define-derived-mode lumenna-trash-mode lumenna-list-mode "Lumenna Trash"
  "Deleted tasks.
RET or r restores one; d deletes it from the trash, asking first.  Erasing
one permanently, from every device's history, is never a key's: choose
it with \\[lumenna-act-at-point].

\\{lumenna-trash-mode-map}"
  (lumenna--mode-name '(command "go-trash"))
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
  (:task_at_point
   ("RET" (word :show_details) lumenna-activate)
   ("c" (kinds "mark_done" "mark_not_done") lumenna-act-done)
   ("e" (command "open-task") lumenna-act-edit)
   ("b" (command "put-in-block") lumenna-act-put-in-block)
   ("m" (command "move-to-project") lumenna-act-move-to-project)
   ("s" (command "make-subtask") lumenna-act-make-subtask)
   ("t" (command "move-to-top") lumenna-act-move-to-top)
   ("w" (command "wait-for") lumenna-act-wait-for)
   ("W" (command "stop-waiting") lumenna-act-stop-waiting)
   ("R" (kinds "reminders") lumenna-act-reminders)
   ("d" (command "trash-task") lumenna-act-delete))
  (:this_list
   ("a" :add_task lumenna-add)
   ("/" :filter_list lumenna-tasks-filter)))

(lumenna-define-keys lumenna-task-mode
  (:this_task
   ("e" :change_field lumenna-task-edit)
   ("c" (kinds "mark_done" "mark_not_done") lumenna-act-done)
   ("b" (command "put-in-block") lumenna-act-put-in-block)
   ("m" (command "move-to-project") lumenna-act-move-to-project)
   ("s" (command "make-subtask") lumenna-act-make-subtask)
   ("t" (command "move-to-top") lumenna-act-move-to-top)
   ("w" (command "wait-for") lumenna-act-wait-for)
   ("W" (command "stop-waiting") lumenna-act-stop-waiting)
   ("R" (kinds "reminders") lumenna-act-reminders)
   ("d" (kinds "delete" "delete_for_good") lumenna-task-delete)))

(lumenna-define-keys lumenna-trash-mode
  (:task_at_point
   ("r" (command "restore-task") lumenna-act-restore)
   ("d" (command "erase-task") lumenna-act-delete)))

(provide 'lumenna-tasks)

;;; lumenna-tasks.el ends here
