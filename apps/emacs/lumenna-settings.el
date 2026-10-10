;;; lumenna-settings.el --- Lumenna's settings, devices, sync and data -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Elijah Massey
;; SPDX-License-Identifier: MPL-2.0

;;; Commentary:

;; The settings core keeps, paired devices and syncing, pairing by comparing
;; three words, and backups, export and import.

;;; Code:

(require 'lumenna)

;;;; Settings

(define-derived-mode lumenna-settings-mode lumenna-list-mode "Lumenna Settings"
  "Settings, one per line.  RET changes one.
The first line opens Devices and sync.  Those that sync to every device
come first, then this device's own.

\\{lumenna-settings-mode-map}"
  (setq-local lumenna--activate #'lumenna--change-setting))

;;;###autoload
(defun lumenna-settings ()
  "Show Lumenna's settings."
  (interactive)
  (lumenna--show-list
   "*Lumenna: Settings*" #'lumenna-settings-mode
   (lambda ()
     (cons "Settings"
           ;; Devices and sync first, as every app's settings have it.
           (cons (list :key 'devices :title "Devices and sync"
                       :value (plist-get (lumenna-call "sync.status") :announcement))
                 (mapcar (lambda (setting)
                           (let* ((value (plist-get setting :value))
                                  (option (seq-find (lambda (o) (equal (plist-get o :id) value))
                                                    (append (plist-get setting :options) nil))))
                             (list :key (plist-get setting :key) :title (plist-get setting :title)
                                   :setting setting
                                   :value (if option (plist-get option :title) value))))
                         ;; `clock' is for clients with no clock of their own; Emacs
                         ;; follows `display-time-24hr-format'.  Shared ones first.
                         (let ((settings (seq-remove (lambda (setting) (equal (plist-get setting :key) "clock"))
                                                     (append (plist-get (lumenna-call "config.get") :settings) nil))))
                           (append (seq-filter (lambda (x) (lumenna--true (plist-get x :syncs))) settings)
                                   (seq-remove (lambda (x) (lumenna--true (plist-get x :syncs))) settings)))))))))

(defun lumenna--change-setting (row)
  "Change the setting in ROW: a choice where it has few values, else typed.
Times are typed as said, 9am or 14:30; core reads and checks every value.
The Devices and sync line opens its own buffer."
  (if (eq (plist-get row :key) 'devices)
      (lumenna-devices)
    (lumenna--change-setting-value row)))

(defun lumenna--change-setting-value (row)
  "Ask for and set a new value for the setting in ROW.
As the setting describes itself: a toggle or a choice is chosen from its
options; a folder is read as a file name; anything else is typed, and the
core reads and checks it."
  (let* ((setting (plist-get row :setting))
         (now (plist-get setting :value))
         (options (mapcar (lambda (o) (cons (plist-get o :title) (plist-get o :id)))
                          (append (plist-get setting :options) nil)))
         (prompt (lumenna--prompt (plist-get setting :hint) (plist-get setting :title)))
         (value (cond (options
                       (cdr (assoc (completing-read (lumenna--with-default prompt (car (rassoc now options)))
                                                    options nil t nil nil (car (rassoc now options)))
                                   options)))
                      ((equal (plist-get setting :kind) "folder")
                       (expand-file-name (read-directory-name prompt now)))
                      (t (read-string prompt now)))))
    (unless (equal value now)
      (lumenna-write "config.set" :key (plist-get setting :key) :value value))))

;;;; Devices and syncing

(define-derived-mode lumenna-devices-mode lumenna-list-mode "Lumenna Devices"
  "Paired devices and how syncing with each last went.  RET renames one.

\\{lumenna-devices-mode-map}"
  (setq-local lumenna--activate (lambda (_row) (lumenna-act-rename))))

(defun lumenna--device-value (device)
  "How syncing with DEVICE last went, in words, never a glyph.
Its platform, then the status the core words for every app."
  (string-join (cons (plist-get device :platform) (append (plist-get device :status) nil)) ", "))

;;;###autoload
(defun lumenna-devices ()
  "Show the paired devices, and how syncing with each is going."
  (interactive)
  (lumenna--show-list
   "*Lumenna: Devices*" #'lumenna-devices-mode
   (lambda ()
     (let* ((status (lumenna-call "sync.status"))
            (none (null (append (plist-get status :devices) nil))))
       ;; With no devices, the line under the heading says so, in the device list's words.
       (setq-local lumenna--empty (and none (plist-get (lumenna-call "device.list") :empty)))
       (cons (string-join (cons (if none "Devices and sync" (plist-get status :announcement))
                                (append (plist-get status :notices) nil))
                          ". ")
             (mapcar (lambda (device)
                       (list :key (plist-get device :node_id) :title (plist-get device :name)
                             :value (lumenna--device-value device) :device device
                             :actions (plist-get device :actions)))
                     (append (plist-get status :devices) nil)))))))

;;;###autoload
(defun lumenna-sync-now ()
  "Sync with every paired device now.  It runs in the background; Emacs stays free."
  (interactive)
  (message "Syncing")
  (jsonrpc-async-request
   (lumenna--connection) 'sync (make-hash-table)
   :success-fn (lambda (report)
                 (lumenna-refresh-all)
                 (lumenna-say
                  (list :announcement (plist-get report :announcement)
                        :notices (append (plist-get report :notices)
                                         (delq nil (mapcar (lambda (peer)
                                                             (when-let* ((error (plist-get peer :error)))
                                                               (format "%s: %s" (plist-get peer :name) error)))
                                                           (append (plist-get report :peers) nil)))))))
   :error-fn (lambda (err) (message "%s" (plist-get err :message)))
   :timeout 300))

;;;; Pairing

(defvar lumenna--pairing nil
  "The pairing under way here: `waiting' to be found, `joining' by a code, or nil.")

(defvar lumenna--next-code nil
  "A code given while waiting, to join with once the wait has ended.")

(defvar lumenna--shown-code nil
  "The code this device shows while it waits; copied only when asked.")

(defun lumenna--pairing-words (&optional name)
  "Pairing's sentences and buttons, the core's (`form.pairing_words').
NAME is how this device is found by the other on its network, from what the
pairing says; Emacs runs where finding each other works."
  (lumenna-words "form.pairing_words" :this_device (or name "this computer") :local t))

(defun lumenna--sentence-message (text)
  "TEXT, one of the core's sentences, as an Emacs message has it: no final stop."
  (string-remove-suffix "." text))

(defun lumenna--read-code ()
  "The other device's code: typed, or, left empty, the latest kill.
That is where a code sent from the other device usually arrives, by way of
the system clipboard.  This device's own code, if it was copied, is never
the other's."
  (let* ((words (lumenna--pairing-words))
         (typed (string-trim (read-string (lumenna--prompt (plist-get words :empty_means)
                                                           (plist-get words :their_code))))))
    (if (not (string-empty-p typed))
        typed
      (let ((killed (ignore-errors (string-trim (current-kill 0 t)))))
        (if (or (null killed) (string-empty-p killed) (equal killed lumenna--shown-code))
            (user-error "%s" (lumenna--sentence-message (plist-get words :need_code)))
          killed)))))

;;;###autoload
(defun lumenna-pair (&optional code)
  "Pair this device with another of yours.
Start pairing on both: on one network they find each other.  Otherwise type
on one the CODE the other shows; with a prefix argument, this asks for it.
Both show three words; say yes only if they are the same on both."
  (interactive (list (and current-prefix-arg (lumenna--read-code))))
  (cond
   ;; A code given while waiting means the other way was chosen: give up the wait, and
   ;; join with the code once it has ended.
   ((and code (eq lumenna--pairing 'waiting))
    (setq lumenna--next-code code)
    (message "%s" (lumenna--sentence-message (plist-get (lumenna--pairing-words) :switching)))
    (lumenna-call "pair.cancel"))
   (lumenna--pairing (user-error "A pairing is already under way; M-x lumenna-pair-cancel ends it"))
   (t (lumenna--start-pairing code))))

(defun lumenna--start-pairing (code)
  "Pair by CODE, or wait to be found when it is nil."
  (setq lumenna--pairing (if code 'joining 'waiting)
        lumenna--shown-code nil)
  (message "%s" (lumenna--sentence-message
                 (plist-get (lumenna--pairing-words) (if code :connecting :opening))))
  (jsonrpc-async-request
   (lumenna--connection) 'pair
   (lumenna--params (list :code (and code (replace-regexp-in-string "[[:space:]]" "" code))
                          :name (system-name)))
   :success-fn (lambda (paired)
                 (setq lumenna--pairing nil)
                 (lumenna-refresh-all)
                 (lumenna-say paired))
   :error-fn (lambda (err)
               (setq lumenna--pairing nil)
               (if lumenna--next-code
                   (let ((next lumenna--next-code))
                     (setq lumenna--next-code nil)
                     (lumenna--start-pairing next))
                 (message "%s" (plist-get err :message))))
   :timeout 700))

(defun lumenna-pair-copy-code ()
  "Copy the code this device shows while it waits, to paste on the other device.
Only when asked: the clipboard is the person's own."
  (interactive)
  (unless (and (eq lumenna--pairing 'waiting) lumenna--shown-code)
    (user-error "No code is shown; this device is not waiting to be found"))
  (kill-new lumenna--shown-code)
  (message "%s" (lumenna--sentence-message (plist-get (lumenna--pairing-words) :copied))))

(defun lumenna-pair-cancel ()
  "Give up the pairing under way."
  (interactive)
  (unless lumenna--pairing (user-error "No pairing is under way"))
  (lumenna-call "pair.cancel"))

(defun lumenna-pairing-notified (params)
  "Act on PARAMS, what a pairing under way says.
The code to give the other device, or the words to compare."
  (cond
   ((plist-get params :code)
    (setq lumenna--shown-code (plist-get params :code))
    (let ((words (lumenna--pairing-words (plist-get params :name))))
      (message "%s %s: %s" (plist-get words :waiting) (plist-get words :my_code) (plist-get params :code))))
   ((plist-get params :words)
    (let* ((words (lumenna--pairing-words))
           (matched (yes-or-no-p
                     (lumenna--prompt (format "%s %s" (plist-get words :match_message)
                                              (string-join (append (plist-get params :words) nil) ", "))
                                      (lumenna-sentence (plist-get words :match_title))))))
      (lumenna-call "pair.confirm" :match (if matched t :json-false))
      (message "%s" (lumenna--sentence-message (plist-get words (if matched :finishing :refusing))))))))

;;;; Backups, export and import

(defun lumenna-back-up-now ()
  "Back up the whole store now, history included."
  (interactive)
  (lumenna-say (lumenna-call "backup")))

(defun lumenna-restore-backup (file)
  "Merge backup FILE into the store.  Nothing here is lost; it only adds."
  (interactive (list (read-file-name "Restore backup: " (alist-get "backup-dir" (lumenna--settings-alist) nil nil #'equal)
                                     nil t)))
  (lumenna-write "restore" :file (expand-file-name file)))

(defun lumenna--settings-alist ()
  "Every setting's key and value, as text."
  (mapcar (lambda (s) (cons (plist-get s :key) (plist-get s :value)))
          (append (plist-get (lumenna-call "config.get") :settings) nil)))

(defun lumenna-export (format file)
  "Write the current state as FORMAT to FILE: nothing from the trash, no history."
  (interactive
   (let* ((formats '(("JSON, complete, can be imported" . "json") ("Markdown checklist" . "markdown")
                     ("Org outline" . "org") ("Calendar file of your blocks" . "ics")))
          (format (cdr (assoc (completing-read "Export as: " formats nil t) formats)))
          (suffix (if (equal format "markdown") "md" format)))
     (list format (read-file-name "Export to: " nil nil nil
                                  (format "Lumenna %s.%s" (format-time-string "%Y-%m-%d") suffix)))))
  (let ((file (expand-file-name file)))
    (when (and (file-exists-p file) (not (yes-or-no-p (format "%s exists; replace it? " file))))
      (user-error "Nothing exported"))
    (lumenna-say (lumenna-call "export" :format format :output file :force (and (file-exists-p file) t)))))

(defun lumenna-import (file)
  "Read FILE, a JSON export or a backup, into the store.  Nothing is removed."
  (interactive (list (read-file-name "Import: " nil nil t)))
  (lumenna-write "import" :file (expand-file-name file)))

;;;; Menus

(lumenna-define-keys lumenna-settings-mode
  ("The setting at point"
   ("RET" "Change it" lumenna-activate))
  ("Devices"
   ("d" "Devices and sync" lumenna-devices))
  ("Data"
   ("b" "Back up now" lumenna-back-up-now)
   ("R" "Restore from a backup" lumenna-restore-backup)
   ("E" "Export" lumenna-export)
   ("I" "Import an export or a backup" lumenna-import)))

(lumenna-define-keys lumenna-devices-mode
  ("The device at point"
   ("r" "Rename" lumenna-act-rename)
   ("d" "Unpair" lumenna-act-delete))
  ("Syncing"
   ("s" "Sync now" lumenna-sync-now)
   ("P" "Pair a device" lumenna-pair)
   ("w" "Copy code" lumenna-pair-copy-code)
   ("c" "Cancel a pairing" lumenna-pair-cancel)))

(provide 'lumenna-settings)

;;; lumenna-settings.el ends here
