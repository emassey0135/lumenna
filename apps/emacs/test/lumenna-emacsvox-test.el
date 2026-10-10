;;; lumenna-emacsvox-test.el --- Lumenna's Emacsvox layer, against Emacsvox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Elijah Massey
;; SPDX-License-Identifier: MPL-2.0

;;; Commentary:

;; Loads Emacsvox's aural layer -- not its speech server -- and asks Emacsvox
;; itself what Lumenna's facts and rules come to.  Skipped where Emacsvox is not
;; found: EMACSVOX names its lisp directory, else ~/emacsvox/lisp.
;;
;;   emacs --batch -Q -L apps/emacs -L apps/emacs/test \
;;     -l apps/emacs/test/lumenna-emacsvox-test.el \
;;     --eval '(ert-run-tests-batch-and-exit "^lumenna-emacsvox-")'

;;; Code:

(require 'lumenna-test)

(defvar lumenna-emacsvox-test--lisp
  (let ((dir (or (getenv "EMACSVOX") (expand-file-name "~/emacsvox/lisp"))))
    (and (file-exists-p (expand-file-name "emacsvox-aural.el" dir)) dir)))

(when lumenna-emacsvox-test--lisp
  (add-to-list 'load-path lumenna-emacsvox-test--lisp)
  (require 'emacsvox-aural-schemes)
  ;; The default scheme names Emacsvox's sound packs.
  (require 'emacsvox-sounds)
  (require 'emacsvox-aural-submission))

(declare-function emacsvox-aural-validate-registry "emacsvox-aural")
(declare-function emacsvox-aural-validate-scheme-registry "emacsvox-aural-schemes")
(declare-function emacsvox-aural-resolve-active "emacsvox-aural-schemes")
(declare-function emacsvox-aural-current-context "emacsvox-aural-schemes")
(declare-function emacsvox-aural-render-plan-after "emacsvox-aural-compiler")
(declare-function emacsvox-aural-action-cue "emacsvox-aural-compiler")
(defvar emacsvox-aural-facts-property)

(defmacro lumenna-emacsvox-test--with-store (&rest body)
  "Run BODY against a fresh store, with Emacsvox's layer loaded."
  (declare (indent 0))
  `(progn
     (skip-unless lumenna-emacsvox-test--lisp)
     (should (featurep 'lumenna-emacsvox))
     (lumenna-test--with-store ,@body)))

(defun lumenna-emacsvox-test--cues (facts)
  "The cues Emacsvox's current rules play after FACTS, in Lumenna's module."
  (let ((plan (emacsvox-aural-resolve-active
               facts (emacsvox-aural-current-context 'lumenna 'state-change))))
    (mapcar #'emacsvox-aural-action-cue (emacsvox-aural-render-plan-after plan))))

(ert-deftest lumenna-emacsvox-registration-is-valid-to-emacsvox ()
  (skip-unless lumenna-emacsvox-test--lisp)
  (should (featurep 'lumenna-emacsvox))
  (should (emacsvox-aural-validate-registry))
  (should (emacsvox-aural-validate-scheme-registry)))

(ert-deftest lumenna-emacsvox-each-event-resolves-to-its-cue ()
  (skip-unless lumenna-emacsvox-test--lisp)
  (dolist (event lumenna-emacsvox-events)
    (pcase-let ((`(,_method ,role ,id ,cue) event))
      (should (equal (lumenna-emacsvox-test--cues (list :role role :event id)) (list cue))))))

(ert-deftest lumenna-emacsvox-a-row-carries-its-role-and-states ()
  (lumenna-emacsvox-test--with-store
    (let ((task (plist-get (lumenna-write "task.add" :text "pay the rent every month") :task)))
      (lumenna-write "task.edit" :id (plist-get task :id) :due "2026-10-01"))
    (lumenna-tasks)
    (lumenna-test--goto "pay the rent")
    (let ((facts (get-text-property (point) emacsvox-aural-facts-property)))
      (should (eq (plist-get facts :role) 'lumenna-task))
      (should (memq 'lumenna-overdue (plist-get facts :states)))
      (should (memq 'lumenna-recurring (plist-get facts :states)))
      (should (equal (plist-get facts :content) "pay the rent")))
    (should (eq (get-text-property (point) 'emacsvox-aural-module) 'lumenna))))

(ert-deftest lumenna-emacsvox-a-line-that-is-no-task-says-it-is-none ()
  (lumenna-emacsvox-test--with-store
    (lumenna)
    (lumenna-test--goto "Today")
    (should-not (get-text-property (point) emacsvox-aural-facts-property))
    (lumenna-help)
    (lumenna-test--goto "  u: Undo")
    (should-not (get-text-property (point) emacsvox-aural-facts-property))))

(ert-deftest lumenna-emacsvox-completing-a-task-is-one-notification-with-its-event ()
  (lumenna-emacsvox-test--with-store
    (lumenna-write "task.add" :text "buy milk")
    (lumenna-tasks)
    (lumenna-test--goto "buy milk")
    (let ((lumenna-announce-function #'lumenna-emacsvox-announce)
          submitted icons)
      (cl-letf (((symbol-function 'emacsvox-aural-submit-notification)
                 (lambda (content &rest keys) (push (cons content keys) submitted)))
                ((symbol-function 'emacspeak-auditory-icon)
                 (lambda (icon) (push icon icons))))
        (lumenna-act-done))
      (should (= (length submitted) 1))
      (pcase-let ((`(,content . ,keys) (car submitted)))
        (should (string-match-p "buy milk" content))
        (should (equal (plist-get keys :facts) '(:role lumenna-task :event lumenna-task-completed)))
        (should (eq (plist-get keys :module) 'lumenna)))
      (should (null icons)))))

(provide (quote lumenna-emacsvox-test))

;;; lumenna-emacsvox-test.el ends here
