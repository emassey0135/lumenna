;;; lumenna-organise.el --- Lumenna's projects, labels and saved filters -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Elijah Massey
;; SPDX-License-Identifier: MPL-2.0

;;; Commentary:

;; Projects, labels and saved filters (§3.4, §6.2): opened with RET, and also
;; made, renamed, reordered and removed.  M-p and M-n move one up or down, as
;; Org moves a subtree.

;;; Code:

(require 'lumenna)

(declare-function lumenna-tasks "lumenna-tasks")

(defun lumenna--sigil (mark name)
  "NAME after MARK, as the filter and quick-add languages read it.
Quoted when it has a space in it."
  (if (string-match-p " " name) (format "%c\"%s\"" mark name) (format "%c%s" mark name)))

(defun lumenna--name ()
  "The name of the project, label or filter at point."
  (plist-get (lumenna-row) :title))

(defun lumenna--listing (method noun)
  (let ((listing (lumenna-call method)))
    (cons (format "%s, %s" noun (plist-get listing :announcement))
          (append (plist-get listing :rows) nil))))

;;;; Projects

(defvar-keymap lumenna-projects-mode-map
  :doc "Keys in the project tree."
  "a" #'lumenna-project-add
  "N" #'lumenna-project-add-inside
  "t" #'lumenna-project-add-task
  "r" #'lumenna-project-rename
  "m" #'lumenna-project-move
  "w" #'lumenna-project-weigh
  "A" #'lumenna-project-archive
  "d" #'lumenna-project-delete
  "M-p" #'lumenna-project-up
  "M-n" #'lumenna-project-down)

(define-derived-mode lumenna-projects-mode lumenna-list-mode "Lumenna Projects"
  "The project tree, with weights and what is archived.
RET shows a project's tasks.

\\{lumenna-projects-mode-map}"
  (setq-local lumenna--menu #'lumenna-projects-menu)
  (setq-local lumenna--activate
              (lambda (row) (let ((name (plist-get row :title)))
                              (lumenna-tasks (lumenna--sigil ?# name) name (concat (lumenna--sigil ?# name) " "))))))

;;;###autoload
(defun lumenna-projects ()
  "Show the project tree (§3.4)."
  (interactive)
  (lumenna--show-list "*Lumenna: Projects*" #'lumenna-projects-mode
                      (lambda () (lumenna--listing "project.list" "Projects"))))

(defun lumenna-project-add (name &optional parent)
  "Add a project called NAME, inside PARENT if given."
  (interactive (list (read-string "New project: ")))
  (when (string-blank-p name) (user-error "A project needs a name"))
  (lumenna-write "project.add" :name name :parent parent))

(defun lumenna-project-add-inside ()
  "Add a project inside the one at point."
  (interactive)
  (let ((parent (lumenna--name)))
    (lumenna-project-add (read-string (format "New project in %s: " parent)) parent)))

(defun lumenna-project-add-task ()
  "Add a task to the project at point."
  (interactive)
  (lumenna-add (concat (lumenna--sigil ?# (lumenna--name)) " ")))

(defun lumenna-project-rename ()
  "Rename the project at point."
  (interactive)
  (let ((name (lumenna--name)))
    (lumenna-write "project.rename" :name name :to (read-string (format "Rename %s to: " name) name))))

(defun lumenna-project-move ()
  "Move the project at point under another, or to the top level."
  (interactive)
  (let* ((name (lumenna--name))
         (others (remove name (mapcar (lambda (row) (plist-get row :title))
                                      (append (plist-get (lumenna-call "project.list") :rows) nil))))
         (choice (completing-read (format "Move %s under: " name) (cons "The top level" others) nil t)))
    (lumenna-write "project.move" :name name :parent (unless (equal choice "The top level") choice))))

(defun lumenna-project-up ()
  "Move the project at point one place up among its siblings."
  (interactive)
  (lumenna-write "project.order" :name (lumenna--name) :direction "up"))

(defun lumenna-project-down ()
  "Move the project at point one place down among its siblings."
  (interactive)
  (lumenna-write "project.order" :name (lumenna--name) :direction "down"))

(defun lumenna-project-weigh ()
  "Set how much the project at point matters now, roughly 0.5 to 2, or inherit.
Not a second priority: priority is how much one task matters; weight is how
much a whole area does (§3.4)."
  (interactive)
  (let* ((name (lumenna--name))
         (text (string-trim (read-string (format "Weight of %s, or inherit: " name) "1.0"))))
    (lumenna-write "project.weight" :name name
                   :value (if (string-equal-ignore-case text "inherit") "inherit"
                            (let ((number (string-to-number text)))
                              (if (> number 0) number (user-error "A weight is a number, such as 1.5, or inherit")))))))

(defun lumenna-project-archive ()
  "Archive the project at point, or unarchive an archived one."
  (interactive)
  (lumenna-write "project.archive" :name (lumenna--name)))

(defun lumenna-project-delete ()
  "Delete the project at point, asking where its tasks go."
  (interactive)
  (let* ((name (lumenna--name))
         (choice (completing-read (format "Delete %s, and its tasks go: " name)
                                  '("To the trash with it" "To the Inbox") nil t)))
    (lumenna-write "project.rm" :name name :keep_tasks (equal choice "To the Inbox"))))

;;;; Labels

(defvar-keymap lumenna-labels-mode-map
  :doc "Keys in the list of labels."
  "a" #'lumenna-label-add
  "t" #'lumenna-label-add-task
  "r" #'lumenna-label-rename
  "m" #'lumenna-label-merge
  "C" #'lumenna-label-colour
  "d" #'lumenna-label-delete
  "M-p" #'lumenna-label-up
  "M-n" #'lumenna-label-down)

(define-derived-mode lumenna-labels-mode lumenna-list-mode "Lumenna Labels"
  "Labels, each with how many open tasks wear it.  RET shows those tasks.

\\{lumenna-labels-mode-map}"
  (setq-local lumenna--menu #'lumenna-labels-menu)
  (setq-local lumenna--activate
              (lambda (row) (let ((name (plist-get row :title)))
                              (lumenna-tasks (lumenna--sigil ?@ name) name (concat (lumenna--sigil ?@ name) " "))))))

;;;###autoload
(defun lumenna-labels ()
  "Show the labels (§3.4)."
  (interactive)
  (lumenna--show-list "*Lumenna: Labels*" #'lumenna-labels-mode
                      (lambda () (lumenna--listing "label.list" "Labels"))))

(defun lumenna-label-add (name)
  "Add a label called NAME."
  (interactive (list (read-string "New label: ")))
  (when (string-blank-p name) (user-error "A label needs a name"))
  (lumenna-write "label.add" :name (string-remove-prefix "@" name)))

(defun lumenna-label-add-task ()
  "Add a task wearing the label at point."
  (interactive)
  (lumenna-add (concat (lumenna--sigil ?@ (lumenna--name)) " ")))

(defun lumenna-label-rename ()
  "Rename the label at point; every task wearing it follows."
  (interactive)
  (let ((name (lumenna--name)))
    (lumenna-write "label.rename" :name name :to (read-string (format "Rename %s to: " name) name))))

(defun lumenna-label-merge ()
  "Fold the label at point into another, for a near-duplicate a typo made."
  (interactive)
  (let* ((name (lumenna--name))
         (others (remove name (mapcar (lambda (row) (plist-get row :title))
                                      (append (plist-get (lumenna-call "label.list") :rows) nil)))))
    (lumenna-write "label.merge" :from name :into (completing-read (format "Merge %s into: " name) others nil t))))

(defun lumenna-label-colour ()
  "Give the label at point a colour, or none.  The name always shows too."
  (interactive)
  (let ((name (lumenna--name)))
    (lumenna-write "label.colour" :name name
                   :colour (read-string (format "Colour for %s, such as teal, or none: " name)))))

(defun lumenna-label-up ()
  "Move the label at point one place up."
  (interactive)
  (lumenna-write "label.order" :name (lumenna--name) :direction "up"))

(defun lumenna-label-down ()
  "Move the label at point one place down."
  (interactive)
  (lumenna-write "label.order" :name (lumenna--name) :direction "down"))

(defun lumenna-label-delete ()
  "Delete the label at point, asking first.  Tasks wearing it stay."
  (interactive)
  (let ((name (lumenna--name)))
    (when (yes-or-no-p (format "Delete %s? Tasks wearing it stay; they just stop showing it. " name))
      (lumenna-write "label.rm" :name name))))

;;;; Saved filters

(defvar-keymap lumenna-filters-mode-map
  :doc "Keys in the list of saved filters."
  "a" #'lumenna-filter-add
  "/" #'lumenna-search
  "r" #'lumenna-filter-rename
  "e" #'lumenna-filter-requery
  "d" #'lumenna-filter-delete
  "M-p" #'lumenna-filter-up
  "M-n" #'lumenna-filter-down)

(define-derived-mode lumenna-filters-mode lumenna-list-mode "Lumenna Filters"
  "Saved filters.  RET shows a filter's tasks.  A filter is kept as typed, so
\"today\" means today whenever it is opened (§6.2).

\\{lumenna-filters-mode-map}"
  (setq-local lumenna--menu #'lumenna-filters-menu)
  (setq-local lumenna--activate (lambda (row) (lumenna-tasks (plist-get row :query) (plist-get row :title)))))

;;;###autoload
(defun lumenna-filters ()
  "Show the saved filters (§3.4)."
  (interactive)
  (lumenna--show-list "*Lumenna: Filters*" #'lumenna-filters-mode
                      (lambda ()
                        (let ((listing (lumenna-call "filter.list")))
                          (cons (format "Saved filters, %s" (plist-get listing :announcement))
                                (mapcar (lambda (saved)
                                          (list :key (plist-get saved :name) :title (plist-get saved :name)
                                                :value (plist-get saved :query) :query (plist-get saved :query)))
                                        (append (plist-get listing :filters) nil)))))))

(defun lumenna-filter-add ()
  "Save a filter under a name."
  (interactive)
  (let ((name (read-string "Name for the filter: ")))
    (when (string-blank-p name) (user-error "A filter needs a name"))
    (lumenna-write "filter.add" :name name
                   :query (lumenna-read-line (format "Query for %s: " name) "filter" nil 'lumenna-filter-history))))

(defun lumenna-filter-rename ()
  "Rename the filter at point."
  (interactive)
  (let ((name (lumenna--name)))
    (lumenna-write "filter.edit" :name name :rename (read-string (format "Rename %s to: " name) name))))

(defun lumenna-filter-requery ()
  "Change the query of the filter at point."
  (interactive)
  (let ((row (lumenna-row)))
    (lumenna-write "filter.edit" :name (plist-get row :title)
                   :query (lumenna-read-line "Query: " "filter" (plist-get row :query) 'lumenna-filter-history))))

(defun lumenna-filter-up ()
  "Move the filter at point one place up."
  (interactive)
  (lumenna-write "filter.order" :name (lumenna--name) :direction "up"))

(defun lumenna-filter-down ()
  "Move the filter at point one place down."
  (interactive)
  (lumenna-write "filter.order" :name (lumenna--name) :direction "down"))

(defun lumenna-filter-delete ()
  "Delete the filter at point, asking first.  The tasks it shows are not touched."
  (interactive)
  (let ((name (lumenna--name)))
    (when (yes-or-no-p (format "Delete the filter %s? The tasks it shows are not touched. " name))
      (lumenna-write "filter.rm" :name name))))

;;;; Menus

(transient-define-prefix lumenna-projects-menu ()
  "What can be done with projects."
  ["The project at point"
   ("RET" "Show its tasks" lumenna-activate)
   ("t" "Add a task to it" lumenna-project-add-task)
   ("N" "Add a project inside it" lumenna-project-add-inside)
   ("r" "Rename" lumenna-project-rename)
   ("m" "Move it under another" lumenna-project-move)
   ("M-p" "Move up" lumenna-project-up)
   ("M-n" "Move down" lumenna-project-down)
   ("w" "Weight" lumenna-project-weigh)
   ("A" "Archive or unarchive" lumenna-project-archive)
   ("d" "Delete" lumenna-project-delete)]
  ["Projects"
   ("a" "Add a project" lumenna-project-add)
   ("u" "Undo" lumenna-undo)
   ("y" "Redo" lumenna-redo)
   ("L" "Lumenna's places" lumenna)])

(transient-define-prefix lumenna-labels-menu ()
  "What can be done with labels."
  ["The label at point"
   ("RET" "Show the tasks wearing it" lumenna-activate)
   ("t" "Add a task wearing it" lumenna-label-add-task)
   ("r" "Rename" lumenna-label-rename)
   ("m" "Merge it into another" lumenna-label-merge)
   ("C" "Colour" lumenna-label-colour)
   ("M-p" "Move up" lumenna-label-up)
   ("M-n" "Move down" lumenna-label-down)
   ("d" "Delete" lumenna-label-delete)]
  ["Labels"
   ("a" "Add a label" lumenna-label-add)
   ("u" "Undo" lumenna-undo)
   ("y" "Redo" lumenna-redo)
   ("L" "Lumenna's places" lumenna)])

(transient-define-prefix lumenna-filters-menu ()
  "What can be done with saved filters."
  ["The filter at point"
   ("RET" "Show its tasks" lumenna-activate)
   ("r" "Rename" lumenna-filter-rename)
   ("e" "Change the query" lumenna-filter-requery)
   ("M-p" "Move up" lumenna-filter-up)
   ("M-n" "Move down" lumenna-filter-down)
   ("d" "Delete" lumenna-filter-delete)]
  ["Filters"
   ("a" "Add a filter" lumenna-filter-add)
   ("/" "Search or filter now" lumenna-search)
   ("u" "Undo" lumenna-undo)
   ("y" "Redo" lumenna-redo)
   ("L" "Lumenna's places" lumenna)])

(provide 'lumenna-organise)

;;; lumenna-organise.el ends here
