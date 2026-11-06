;;; lumenna-organise.el --- Lumenna's projects, labels and saved filters -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Elijah Massey
;; SPDX-License-Identifier: MPL-2.0

;;; Commentary:

;; Projects, labels and saved filters: opened with RET, and also made,
;; renamed, reordered and removed.  Each is a part of the app's places, as
;; every app's sidebar has them, drawn in a buffer of its own beside the
;; places' buffer; M-p and M-n move one up or down, as Org moves a subtree.

;;; Code:

(require 'lumenna)

;; Every action on a project, label or saved filter is the row's own; these
;; are the keys for them.  Adding one is its heading's action.
(lumenna-define-action lumenna-act-new-inside ("new_inside")
  "Add a project inside the one at point.")
(lumenna-define-action lumenna-act-move-under ("move_under")
  "Move the project at point under another.")
(lumenna-define-action lumenna-act-weight ("weight")
  "Set how much the project at point matters now.
Not a second priority: priority is how much one task matters; weight is how
much a whole area does.")
(lumenna-define-action lumenna-act-archive ("archive" "unarchive")
  "Archive the project at point, or unarchive an archived one.")
(lumenna-define-action lumenna-act-merge ("merge_into")
  "Fold the label at point into another, for a near-duplicate a typo made.")
(lumenna-define-action lumenna-act-colour ("colour")
  "Give the project, label or saved filter at point a colour, or none.
The name always shows too.")
(lumenna-define-action lumenna-act-change-query ("change_query")
  "Change the query of the saved filter at point.")

(defvar-local lumenna--group nil
  "The places' heading this buffer lists what is under.
\"Projects\", \"Labels\" or \"Filters\".")

(defun lumenna--group-rows ()
  "What sits under this buffer's heading in the places, the heading first."
  (let* ((all (cdr (lumenna--place-rows)))
         (start (seq-position all lumenna--group
                              (lambda (row group) (equal (plist-get (plist-get row :place) :Group) group))))
         (rows (and start (cdr (nthcdr start all)))))
    (setq-local lumenna--empty nil)
    (cons (or (and start (nth start all)) lumenna--group)
          (seq-take-while (lambda (row) (> (or (plist-get row :depth) 0) 0)) rows))))

(define-derived-mode lumenna-group-mode lumenna-home-mode "Lumenna"
  "Part of the places: the projects, the labels or the saved filters."
  (setq-local lumenna--draw #'lumenna--group-rows))

(defun lumenna--show-group (group name mode)
  "Show GROUP of the places in a buffer named after NAME, a word, in MODE."
  (lumenna--home-buffer)
  (let ((buffer (get-buffer-create (lumenna--buffer-name (lumenna-word name)))))
    (with-current-buffer buffer
      (unless (derived-mode-p mode)
        (funcall mode)
        (setq lumenna--group group)
        (lumenna--attach 0))
      (setq lumenna--view (buffer-local-value 'lumenna--view (lumenna--home-buffer))
            lumenna--lines (buffer-local-value 'lumenna--lines (lumenna--home-buffer)))
      (lumenna--redraw)
      (goto-char (point-min))
      (forward-line 1))
    (pop-to-buffer-same-window buffer)))

(defun lumenna--add-from-heading ()
  "Do what this buffer's heading offers: add a project, label or filter."
  (let ((heading (save-excursion (goto-char (point-min)) (lumenna--row-here))))
    (lumenna-act (lumenna-heading-action lumenna--group) heading)))

(declare-function lumenna-open-place "lumenna-tasks" (place))

(defun lumenna--add-task-here ()
  "Add a task to the place at point, its line starting as that place's does."
  (let ((place (plist-get (plist-get (lumenna-row) :place) :Place)))
    (unless (and place (not (stringp place))) (lumenna--refusal :nothing_to_do))
    (with-current-buffer (save-window-excursion (lumenna-open-place place) (current-buffer))
      (lumenna-add))))

;;;; Projects

(define-derived-mode lumenna-projects-mode lumenna-group-mode "Lumenna Projects"
  "The project tree, with weights and what is archived.
RET shows a project's tasks.

\\{lumenna-projects-mode-map}"
  (lumenna--mode-name :projects))

;;;###autoload
(defun lumenna-projects ()
  "Show the project tree."
  (interactive)
  (lumenna--show-group "Projects" :projects #'lumenna-projects-mode))

(defun lumenna-project-add ()
  "Add a project, as the Projects heading offers."
  (interactive)
  (lumenna--add-from-heading))

(defun lumenna-project-add-task ()
  "Add a task to the project at point."
  (interactive)
  (lumenna--add-task-here))

;;;; Labels

(define-derived-mode lumenna-labels-mode lumenna-group-mode "Lumenna Labels"
  "Labels, each with how many open tasks wear it.  RET shows those tasks.

\\{lumenna-labels-mode-map}"
  (lumenna--mode-name :labels))

;;;###autoload
(defun lumenna-labels ()
  "Show the labels."
  (interactive)
  (lumenna--show-group "Labels" :labels #'lumenna-labels-mode))

(defun lumenna-label-add ()
  "Add a label, as the Labels heading offers."
  (interactive)
  (lumenna--add-from-heading))

(defun lumenna-label-add-task ()
  "Add a task wearing the label at point."
  (interactive)
  (lumenna--add-task-here))

;;;; Saved filters

(define-derived-mode lumenna-filters-mode lumenna-group-mode "Lumenna Filters"
  "Saved filters.  RET shows a filter's tasks.
A filter is kept as typed, so \"today\" means today whenever it is opened.

\\{lumenna-filters-mode-map}"
  (lumenna--mode-name '(word :filters)))

;;;###autoload
(defun lumenna-filters ()
  "Show the saved filters."
  (interactive)
  (lumenna--show-group "Filters" '(word :filters) #'lumenna-filters-mode))

(defun lumenna-filter-add ()
  "Save a filter under a name, as the Saved Filters heading offers."
  (interactive)
  (lumenna--add-from-heading))

;;;; Menus

(lumenna-define-keys lumenna-projects-mode
  (:project_at_point
   ("RET" (word :show_its_tasks) lumenna-activate)
   ("a" (command "new-project") lumenna-project-add)
   ("t" (word :add_task_to_it) lumenna-project-add-task)
   ("N" (kinds "new_inside") lumenna-act-new-inside)
   ("r" (kinds "rename") lumenna-act-rename)
   ("m" (kinds "move_under") lumenna-act-move-under)
   ("T" (kinds "move_to_top_level") lumenna-act-move-to-top)
   ("M-p" (kinds "move_up") lumenna-act-move-up)
   ("M-n" (kinds "move_down") lumenna-act-move-down)
   ("w" (kinds "weight") lumenna-act-weight)
   ("A" (kinds "archive" "unarchive") lumenna-act-archive)
   ("C" (kinds "colour") lumenna-act-colour)
   ("d" (kinds "delete") lumenna-act-delete)))

(lumenna-define-keys lumenna-labels-mode
  (:label_at_point
   ("RET" (word :show_tasks_wearing_it) lumenna-activate)
   ("a" (command "new-label") lumenna-label-add)
   ("t" (word :add_task_wearing_it) lumenna-label-add-task)
   ("r" (kinds "rename") lumenna-act-rename)
   ("m" (kinds "merge_into") lumenna-act-merge)
   ("C" (kinds "colour") lumenna-act-colour)
   ("M-p" (kinds "move_up") lumenna-act-move-up)
   ("M-n" (kinds "move_down") lumenna-act-move-down)
   ("d" (kinds "delete") lumenna-act-delete)))

(lumenna-define-keys lumenna-filters-mode
  (:filter_at_point
   ("RET" (word :show_its_tasks) lumenna-activate)
   ("a" (command "new-filter") lumenna-filter-add)
   ("r" (kinds "rename") lumenna-act-rename)
   ("e" (kinds "change_query") lumenna-act-change-query)
   ("C" (kinds "colour") lumenna-act-colour)
   ("M-p" (kinds "move_up") lumenna-act-move-up)
   ("M-n" (kinds "move_down") lumenna-act-move-down)
   ("d" (kinds "delete") lumenna-act-delete)))

(provide 'lumenna-organise)

;;; lumenna-organise.el ends here
