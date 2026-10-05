;;; lumenna-emacsvox.el --- Lumenna's meanings, for Emacsvox's aural presentation -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Elijah Massey
;; SPDX-License-Identifier: MPL-2.0

;;; Commentary:

;; Emacsvox decides how things sound from what they mean.  This tells it:
;;
;; - each line is a task, a block or a sitting, with its states -- completed,
;;   overdue, recurring, blocked, a timer running -- as semantic facts on the
;;   text, so moving onto it is presented as that object;
;; - each change is an event -- a task completed, deleted, a timer started --
;;   submitted on Emacsvox's notification lane with its facts, where its rules
;;   add a sound.
;;
;; The rules are a module fragment, so a person changes them in Aural Home
;; (C-e H) like any integration's, and C-e E explains a line.  Loaded only when
;; Emacsvox's aural layer is; without it, everything here is unused.

;;; Code:

(require 'lumenna)
(require 'emacsvox-aural)
(require 'emacsvox-aural-source)
(require 'emacsvox-aural-submission)

(defvar emacsvox-speak-messages)

(defconst lumenna-emacsvox-semantics
  '((lumenna-task :kind role :summary "A Lumenna task")
    (lumenna-block :kind role :summary "A Lumenna time block in a day")
    (lumenna-sitting :kind role :summary "One sitting of a task in a Lumenna block")
    (lumenna-completed :kind state :summary "A finished Lumenna task"
                       :roles (lumenna-task lumenna-sitting))
    (lumenna-overdue :kind state :summary "A Lumenna task due in the past and not finished"
                     :roles (lumenna-task lumenna-sitting))
    (lumenna-recurring :kind state :summary "A Lumenna task that repeats" :roles (lumenna-task))
    (lumenna-blocked :kind state :summary "A Lumenna task waiting for another" :roles (lumenna-task))
    (lumenna-running :kind state :summary "A Lumenna sitting whose timer is running" :roles (lumenna-sitting))
    (lumenna-task-completed :kind event :summary "A Lumenna task was completed"
                            :roles (lumenna-task))
    (lumenna-deleted :kind event :summary "A Lumenna task, block or sitting was deleted"
                     :roles (lumenna-task lumenna-block lumenna-sitting))
    (lumenna-timer-started :kind event :summary "A Lumenna sitting's timer started" :roles (lumenna-sitting))
    (lumenna-timer-stopped :kind event :summary "A Lumenna sitting's timer stopped" :roles (lumenna-sitting)))
  "What Lumenna's facts mean, registered with Emacsvox and owned by Lumenna.")

(defconst lumenna-emacsvox-events
  '(("task.done" lumenna-task lumenna-task-completed task-done)
    ("task.rm" lumenna-task lumenna-deleted delete-object)
    ("task.erase" lumenna-task lumenna-deleted delete-object)
    ("block.rm" lumenna-block lumenna-deleted delete-object)
    ("unassign" lumenna-sitting lumenna-deleted delete-object)
    ("start" lumenna-sitting lumenna-timer-started open-object)
    ("stop" lumenna-sitting lumenna-timer-stopped close-object))
  "Each change, by RPC method: the role it is about, its event, and its cue.")

(defun lumenna-emacsvox--register ()
  "Register Lumenna's semantics and its rules, once."
  (dolist (definition lumenna-emacsvox-semantics)
    (unless (emacsvox-aural-semantic (car definition))
      (apply #'emacsvox-aural-register-semantic (car definition) :owner 'lumenna (cdr definition))))
  (unless (gethash 'lumenna-events emacsvox-aural-module-fragment-registry)
    (emacsvox-aural-register-module-fragment
     'lumenna
     `(:schema-version 1
       :id lumenna-events
       :summary "Sounds for what happens to Lumenna's tasks and timers"
       :rules
       ,(mapcar (lambda (event)
                  (pcase-let ((`(,method ,role ,id ,cue) event))
                    `(:id ,(intern (format "lumenna-%s-cue" (replace-regexp-in-string "\\." "-" method)))
                      :match (:role ,role :module lumenna :event ,id)
                      :render (:after ((:id ,(intern (format "lumenna-%s-sound" (replace-regexp-in-string "\\." "-" method)))
                                        :kind cue :cue ,cue))))))
                lumenna-emacsvox-events))
     :source "lumenna-emacsvox")))

(defconst lumenna-emacsvox--roles
  '(("task" . lumenna-task) ("block" . lumenna-block) ("assignment" . lumenna-sitting))
  "A row's role, as Emacsvox is told it.")

(defun lumenna-emacsvox--facts (row)
  "ROW's semantic facts, or nil for a row that is not a task, block or sitting."
  (when-let* ((role (cdr (assoc (or (plist-get row :role) "task") lumenna-emacsvox--roles))))
    (let ((states (append (plist-get row :state) nil))
          (sitting (plist-get row :sitting)))
      (list :role role
            :states (delq nil (list (and (lumenna--true (plist-get row :checked)) 'lumenna-completed)
                                    (and (member "overdue" states) 'lumenna-overdue)
                                    (and (eq role 'lumenna-task) (member "recurring" states) 'lumenna-recurring)
                                    (and (eq role 'lumenna-task) (member "blocked" states) 'lumenna-blocked)
                                    (and sitting (equal (plist-get sitting :status) "in progress") 'lumenna-running)))
            :content (plist-get row :title)))))

(defun lumenna-emacsvox--annotate (start end row)
  "Put ROW's facts on its line, START to END, so Emacsvox presents it as that object."
  (when-let* ((facts (lumenna-emacsvox--facts row)))
    (add-text-properties start end
                         (list emacsvox-aural-facts-property facts
                               emacsvox-aural-module-property 'lumenna
                               emacsvox-aural-object-property (or (plist-get row :id) (plist-get row :title))))))

(defvar lumenna-emacsvox--event nil
  "The facts of the change about to be announced, set just before it is.")

(defun lumenna-emacsvox--changed (method _result)
  (setq lumenna-emacsvox--event
        (when-let* ((event (assoc method lumenna-emacsvox-events)))
          (list :role (nth 1 event) :event (nth 2 event)))))

(defun lumenna-emacsvox-announce (announcement notices)
  "Say ANNOUNCEMENT and NOTICES on Emacsvox's notification lane, with the
change's facts so its rules can add a sound; and show them, unspoken, in the
echo area, so they are said once."
  (let ((text (string-join (seq-remove #'string-empty-p (cons announcement notices)) ". "))
        (facts lumenna-emacsvox--event))
    (setq lumenna-emacsvox--event nil)
    (unless (string-empty-p text)
      (emacsvox-aural-submit-notification text :facts facts :module 'lumenna :occasion 'state-change)
      (let ((emacsvox-speak-messages nil))
        (message "%s" text)))))

(lumenna-emacsvox--register)
(add-hook 'lumenna-row-inserted-functions #'lumenna-emacsvox--annotate)
(add-hook 'lumenna-changed-functions #'lumenna-emacsvox--changed)
(setq lumenna-announce-function #'lumenna-emacsvox-announce)

(provide 'lumenna-emacsvox)

;;; lumenna-emacsvox.el ends here
