;;; lumenna-voice.el --- Lumenna's faces as voices, and its sounds, for Emacspeak -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Elijah Massey
;; SPDX-License-Identifier: MPL-2.0

;;; Commentary:

;; Two things for speech systems that read faces and play sounds.
;;
;; Faces to voices: done tasks in a lighter voice, overdue ones in a livelier
;; one, headings bolder.  `voice-setup-add-map' is the same function in
;; Emacspeak and Emacsvox, so this one map serves both; every state is also
;; said in words, so nothing depends on hearing the voice change.
;;
;; Sounds, for Emacspeak: completing, deleting, starting and stopping a timer.
;; Under Emacsvox the sounds come from `lumenna-emacsvox' instead, through its
;; aural rules, which a person can change -- so they are not played here too.

;;; Code:

(require 'lumenna)

(defconst lumenna-voice-map
  '((lumenna-heading voice-bolden)
    (lumenna-done voice-lighten)
    (lumenna-overdue voice-animate)
    (lumenna-quiet voice-smoothen)
    (lumenna-now voice-bolden)
    (lumenna-running voice-animate))
  "Lumenna's faces, and the voice each is read in.")

(declare-function voice-setup-add-map "voice-setup" (fv-alist &optional origin))

(with-eval-after-load 'voice-setup
  (voice-setup-add-map lumenna-voice-map))

(defconst lumenna-voice-sounds
  '(("task.done" . task-done)
    ("task.rm" . delete-object)
    ("task.erase" . delete-object)
    ("block.rm" . delete-object)
    ("unassign" . delete-object)
    ("start" . open-object)
    ("stop" . close-object))
  "The sound each kind of change plays, by its RPC method.")

(defun lumenna-voice--sound (method _result)
  "Play METHOD's sound in Emacspeak, if there is one and Emacsvox is not doing it."
  (when-let* ((icon (cdr (assoc method lumenna-voice-sounds))))
    (when (and (fboundp 'emacspeak-auditory-icon) (not (featurep 'lumenna-emacsvox)))
      (funcall 'emacspeak-auditory-icon icon))))

(add-hook 'lumenna-changed-functions #'lumenna-voice--sound)

(provide 'lumenna-voice)

;;; lumenna-voice.el ends here
