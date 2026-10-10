;;; lumenna-organise.el --- Lumenna's projects, labels and saved filters -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Elijah Massey
;; SPDX-License-Identifier: MPL-2.0

;;; Commentary:

;; Projects, labels and saved filters: opened with RET, and also
;; made, renamed, reordered and removed.  M-p and M-n move one up or down, as
;; Org moves a subtree.

;;; Code:

(require 'lumenna)

(declare-function lumenna-tasks "lumenna-tasks")

(defun lumenna--name ()
  "The name of the project, label or filter at point."
  (plist-get (lumenna-row) :title))

(defun lumenna--listing (method noun)
  "The heading and rows METHOD lists, the heading starting with NOUN."
  (let ((listing (lumenna-call method)))
    (lumenna-listing noun listing)))

;; Every action on a project, label or saved filter is the row's own; these
;; are the keys for them.  Adding one is its heading's action in `places'.
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
  "Give the label at point a colour, or none.  The name always shows too.")
(lumenna-define-action lumenna-act-change-query ("change_query")
  "Change the query of the saved filter at point.")

(defun lumenna--add-from-heading (group)
  "Do what the places' GROUP heading offers: add a project, label or filter."
  (lumenna-act (lumenna-heading-action group)))

;;;; Projects

(define-derived-mode lumenna-projects-mode lumenna-list-mode "Lumenna Projects"
  "The project tree, with weights and what is archived.
RET shows a project's tasks.

\\{lumenna-projects-mode-map}"
  (setq-local lumenna--activate (lambda (row) (lumenna-open-project (plist-get row :title)))))

(defun lumenna-open-project (name)
  "Show the tasks of project NAME; a task added there starts in it."
  (let ((reference (lumenna-project-reference name)))
    (lumenna-tasks reference name (concat reference " "))))

;;;###autoload
(defun lumenna-projects ()
  "Show the project tree."
  (interactive)
  (lumenna--show-list "*Lumenna: Projects*" #'lumenna-projects-mode
                      (lambda () (lumenna--listing "project.list" "Projects"))))

(defun lumenna-project-add ()
  "Add a project, as the Projects heading offers."
  (interactive)
  (lumenna--add-from-heading "Projects"))

(defun lumenna-project-add-task ()
  "Add a task to the project at point."
  (interactive)
  (lumenna-add (concat (lumenna-project-reference (lumenna--name)) " ")))

;;;; Labels

(define-derived-mode lumenna-labels-mode lumenna-list-mode "Lumenna Labels"
  "Labels, each with how many open tasks wear it.  RET shows those tasks.

\\{lumenna-labels-mode-map}"
  (setq-local lumenna--activate (lambda (row) (lumenna-open-label (plist-get row :title)))))

(defun lumenna-open-label (name)
  "Show the tasks wearing label NAME; a task added there wears it."
  (let ((reference (lumenna-label-reference name)))
    (lumenna-tasks reference name (concat reference " "))))

;;;###autoload
(defun lumenna-labels ()
  "Show the labels."
  (interactive)
  (lumenna--show-list "*Lumenna: Labels*" #'lumenna-labels-mode
                      (lambda () (lumenna--listing "label.list" "Labels"))))

(defun lumenna-label-add ()
  "Add a label, as the Labels heading offers."
  (interactive)
  (lumenna--add-from-heading "Labels"))

(defun lumenna-label-add-task ()
  "Add a task wearing the label at point."
  (interactive)
  (lumenna-add (concat (lumenna-label-reference (lumenna--name)) " ")))

;;;; Saved filters

(define-derived-mode lumenna-filters-mode lumenna-list-mode "Lumenna Filters"
  "Saved filters.  RET shows a filter's tasks.
A filter is kept as typed, so \"today\" means today whenever it is opened.

\\{lumenna-filters-mode-map}"
  (setq-local lumenna--activate (lambda (row) (lumenna-tasks (plist-get row :query) (plist-get row :title)))))

;;;###autoload
(defun lumenna-filters ()
  "Show the saved filters."
  (interactive)
  (lumenna--show-list "*Lumenna: Filters*" #'lumenna-filters-mode
                      (lambda ()
                        (let ((listing (lumenna-call "filter.list")))
                          (lumenna-listing
                                "Saved filters" listing
                                (mapcar (lambda (saved)
                                          (list :key (plist-get saved :name) :title (plist-get saved :name)
                                                :value (plist-get saved :query) :query (plist-get saved :query)
                                                :actions (plist-get saved :actions)))
                                        (append (plist-get listing :filters) nil)))))))

(defun lumenna-filter-add ()
  "Save a filter under a name, as the Saved Filters heading offers."
  (interactive)
  (lumenna--add-from-heading "Filters"))

(defun lumenna--new-filter-form (&rest _)
  "The new saved filter's form: its name, then its query, which TAB completes."
  (pcase-let* ((`(,naming ,querying) (append (lumenna-words "form.new_filter") nil))
               (ask (lambda (q) (lumenna--prompt (lumenna--question-title q) (plist-get q :hint) (plist-get q :label))))
               (name (read-string (funcall ask naming))))
    (lumenna-write "filter.add" :name name
                   :query (lumenna-read-line (funcall ask querying) "filter" nil 'lumenna-filter-history))))

(lumenna-define-form "filter" "new" #'lumenna--new-filter-form)

;;;; Menus

(lumenna-define-keys lumenna-projects-mode
  ("The project at point"
   ("RET" "Show its tasks" lumenna-activate)
   ("a" "New Project" lumenna-project-add)
   ("t" "Add a task to it" lumenna-project-add-task)
   ("N" "New Project Inside" lumenna-act-new-inside)
   ("r" "Rename" lumenna-act-rename)
   ("m" "Move Under" lumenna-act-move-under)
   ("T" "Move to Top Level" lumenna-act-move-to-top)
   ("M-p" "Move Up" lumenna-act-move-up)
   ("M-n" "Move Down" lumenna-act-move-down)
   ("w" "Weight" lumenna-act-weight)
   ("A" "Archive or Unarchive" lumenna-act-archive)
   ("d" "Delete" lumenna-act-delete)))

(lumenna-define-keys lumenna-labels-mode
  ("The label at point"
   ("RET" "Show the tasks wearing it" lumenna-activate)
   ("a" "New Label" lumenna-label-add)
   ("t" "Add a task wearing it" lumenna-label-add-task)
   ("r" "Rename" lumenna-act-rename)
   ("m" "Merge Into" lumenna-act-merge)
   ("C" "Colour" lumenna-act-colour)
   ("M-p" "Move Up" lumenna-act-move-up)
   ("M-n" "Move Down" lumenna-act-move-down)
   ("d" "Delete" lumenna-act-delete)))

(lumenna-define-keys lumenna-filters-mode
  ("The filter at point"
   ("RET" "Show its tasks" lumenna-activate)
   ("a" "New Saved Filter" lumenna-filter-add)
   ("r" "Rename" lumenna-act-rename)
   ("e" "Change Query" lumenna-act-change-query)
   ("M-p" "Move Up" lumenna-act-move-up)
   ("M-n" "Move Down" lumenna-act-move-down)
   ("d" "Delete" lumenna-act-delete)))

(provide 'lumenna-organise)

;;; lumenna-organise.el ends here
