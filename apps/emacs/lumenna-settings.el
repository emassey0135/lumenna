;;; lumenna-settings.el --- Lumenna's settings, devices, sync and data -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Elijah Massey
;; SPDX-License-Identifier: MPL-2.0

;;; Commentary:

;; The settings core keeps, paired devices and syncing, pairing by comparing
;; three words, and backups, export and import.

;;; Code:

(require 'lumenna)

;;;; Settings

(defconst lumenna--setting-names
  '(("cascade-complete-subtasks" "Completing a task completes its subtasks" ("true" . "Yes") ("false" . "No"))
    ("day-start" "Day starts")
    ("day-end" "Day ends")
    ("all-day-reminder-hour" "All-day reminders at")
    ("verbosity" "Announcements" ("full" . "Full sentences") ("terse" . "Terse"))
    ("week-start" "Week starts on" ("monday" . "Monday") ("tuesday" . "Tuesday") ("wednesday" . "Wednesday")
     ("thursday" . "Thursday") ("friday" . "Friday") ("saturday" . "Saturday") ("sunday" . "Sunday"))
    ("backup-every" "Automatic backups" ("12h" . "Every 12 hours") ("1d" . "Every day") ("7d" . "Every week") ("off" . "Off"))
    ("backup-keep" "Backups kept")
    ("backup-dir" "Backups go to"))
  "Each setting's name in words, and the values to choose from when there are few.")

(define-derived-mode lumenna-settings-mode lumenna-list-mode "Lumenna Settings"
  "Settings, one per line.  RET changes one.
The first line opens Devices and sync.  The next six sync to every device;
the backup settings are this device's alone.

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
                           (let* ((key (plist-get setting :key))
                                  (named (assoc key lumenna--setting-names))
                                  (value (plist-get setting :value)))
                             (list :key key :title (or (nth 1 named) key) :raw value
                                   :value (or (cdr (assoc value (nthcdr 2 named))) value))))
                         (append (plist-get (lumenna-call "config.get") :settings) nil)))))))

(defun lumenna--change-setting (row)
  "Change the setting in ROW: a choice where it has few values, else typed.
Times are typed as said, 9am or 14:30; core reads and checks every value.
The Devices and sync line opens its own buffer."
  (if (eq (plist-get row :key) 'devices)
      (lumenna-devices)
    (lumenna--change-setting-value row)))

(defun lumenna--change-setting-value (row)
  "Ask for and set a new value for the setting in ROW."
  (let* ((key (plist-get row :key))
         (choices (mapcar (lambda (pair) (cons (cdr pair) (car pair))) (nthcdr 2 (assoc key lumenna--setting-names))))
         (value (if choices
                    (cdr (assoc (completing-read (format "%s: " (plist-get row :title)) choices nil t
                                                 nil nil (car (rassoc (plist-get row :raw) choices)))
                                choices))
                  (read-string (format "%s: " (plist-get row :title)) (plist-get row :raw)))))
    (unless (equal value (plist-get row :raw))
      (lumenna-write "config.set" :key key :value value))))

;;;; Devices and syncing

(define-derived-mode lumenna-devices-mode lumenna-list-mode "Lumenna Devices"
  "Paired devices and how syncing with each last went.  RET renames one.

\\{lumenna-devices-mode-map}"
  (setq-local lumenna--activate (lambda (_row) (lumenna-device-rename))))

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
     (let ((status (lumenna-call "sync.status")))
       (cons (string-join (cons (plist-get status :announcement) (append (plist-get status :notices) nil)) ". ")
             (mapcar (lambda (device)
                       (list :key (plist-get device :node_id) :title (plist-get device :name)
                             :value (lumenna--device-value device) :device device))
                     (append (plist-get status :devices) nil)))))))

(defun lumenna-device-rename ()
  "Rename the device at point."
  (interactive)
  (let ((device (plist-get (lumenna-row) :device)))
    (lumenna-write "device.rename" :device (plist-get device :node_id)
                   :name (read-string (format "Rename %s to: " (plist-get device :name)) (plist-get device :name)))))

(defun lumenna-device-unpair ()
  "Stop syncing with the device at point.  It keeps what it already has."
  (interactive)
  (let ((device (plist-get (lumenna-row) :device)))
    (when (lumenna--true (plist-get device :this_device)) (user-error "That is this device"))
    (when (yes-or-no-p (format "%s keeps what it already has: this is for a device you replaced, not one that was stolen.  Stop syncing with it? "
                               (plist-get device :name)))
      (lumenna-write "device.unpair" :device (plist-get device :node_id)))))

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

;;;###autoload
(defun lumenna-pair (&optional code)
  "Pair this device with another of yours.
Start pairing on both: on one network they find each other.  Otherwise type
on one the CODE the other shows; with a prefix argument, this asks for it.
Both show three words; say yes only if they are the same on both."
  (interactive (list (and current-prefix-arg (read-string "The other device's code: "))))
  (cond
   ;; A code given while waiting means the other way was chosen: give up the wait, and
   ;; join with the code once it has ended.
   ((and code (eq lumenna--pairing 'waiting))
    (setq lumenna--next-code code)
    (message "Stopping the wait, then connecting with this code")
    (lumenna-call "pair.cancel"))
   (lumenna--pairing (user-error "A pairing is already under way; M-x lumenna-pair-cancel ends it"))
   (t (lumenna--start-pairing code))))

(defun lumenna--start-pairing (code)
  "Pair by CODE, or wait to be found when it is nil."
  (setq lumenna--pairing (if code 'joining 'waiting))
  (message (if code "Connecting to the other device" "Waiting for the other device"))
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
    (kill-new (plist-get params :code))
    (message "Waiting to pair, as %s. On the other device, pair too while on this network, or give it this code, which is copied: %s"
             (plist-get params :name) (plist-get params :code)))
   ((plist-get params :words)
    (let ((matched (yes-or-no-p
                    (format "The words are: %s. Do the same three words show on the other device? "
                            (string-join (append (plist-get params :words) nil) ", ")))))
      (lumenna-call "pair.confirm" :match (if matched t :json-false))
      (message (if matched "Finishing" "Refusing"))))))

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
   ("r" "Rename" lumenna-device-rename)
   ("d" "Stop syncing with it" lumenna-device-unpair))
  ("Syncing"
   ("s" "Sync now" lumenna-sync-now)
   ("P" "Pair a device" lumenna-pair)
   ("c" "Cancel a pairing" lumenna-pair-cancel)))

(provide 'lumenna-settings)

;;; lumenna-settings.el ends here
