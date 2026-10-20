;;; lumenna-settings.el --- Lumenna's settings, devices, sync and data -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Elijah Massey
;; SPDX-License-Identifier: MPL-2.0

;;; Commentary:

;; The settings core keeps, paired devices and syncing, pairing by comparing
;; three words, and backups, export and import.  Settings and the devices are
;; screens of the app's, and pairing is its; a backup, an export and an import
;; are the store's own operations, on files Emacs names.

;;; Code:

(require 'lumenna)

;;;; Settings

(define-derived-mode lumenna-settings-mode lumenna-list-mode "Lumenna Settings"
  "Settings, one per line.  RET changes one.
The first line opens Devices and sync.  Those that sync to every device
come first, then this device's own.

\\{lumenna-settings-mode-map}"
  (setq-local lumenna--draw #'lumenna--setting-rows)
  (setq-local lumenna--activate #'lumenna--change-setting))

(defun lumenna--setting-value (setting typed)
  "What SETTING holds, as its line says it: TYPED, or an option's own words."
  (let ((option (seq-find (lambda (o) (equal (plist-get o :id) typed)) (append (plist-get setting :options) nil))))
    (if option (plist-get option :title) typed)))

(defun lumenna--setting-rows ()
  "Devices and sync, then every setting, shared ones first."
  (let* ((settings (append (plist-get (plist-get lumenna--view :settings) :settings) nil))
         ;; `clock' is for clients with no clock of their own; Emacs follows
         ;; `display-time-24hr-format'.
         (settings (seq-remove (lambda (s) (equal (plist-get (plist-get s :setting) :key) "clock")) settings))
         (shared (seq-filter (lambda (s) (lumenna--true (plist-get (plist-get s :setting) :syncs))) settings))
         (own (seq-remove (lambda (s) (lumenna--true (plist-get (plist-get s :setting) :syncs))) settings))
         (status (plist-get (plist-get lumenna--view :devices) :status)))
    (setq-local lumenna--empty nil)
    (cons "Settings"
          ;; Devices and sync first, as every app's settings have it.
          (cons (list :key "devices" :title "Devices and sync"
                      :text (lumenna--heading "Devices and sync" status))
                (mapcar (lambda (shown)
                          (let ((setting (plist-get shown :setting)))
                            (list :key (plist-get setting :key) :setting setting :problem (plist-get shown :problem)
                                  :text (format "%s, %s" (plist-get setting :title)
                                                (lumenna--setting-value setting (plist-get shown :typed))))))
                        (append shared own))))))

;;;###autoload
(defun lumenna-settings ()
  "Show Lumenna's settings."
  (interactive)
  (lumenna--show-screen "*Lumenna: Settings*" #'lumenna-settings-mode
                        (list (list :Settings "Open") (list :Devices "Show"))))

(defun lumenna--change-setting (row)
  "Change the setting in ROW; the Devices and sync line opens its own buffer."
  (if (equal (plist-get row :key) "devices")
      (lumenna-devices)
    (lumenna--change-setting-value row)))

(defun lumenna--set (key value)
  "Apply VALUE to the setting KEY, the app's way, and wait for it."
  (lumenna-send (list :Settings (list :Set (list :key key :value value))))
  (lumenna--follow))

(defun lumenna--setting-problem (key)
  "Why the line last applied to setting KEY was refused, if it was."
  (plist-get (seq-find (lambda (s) (equal (plist-get (plist-get s :setting) :key) key))
                       (append (plist-get (plist-get lumenna--view :settings) :settings) nil))
             :problem))

(defun lumenna--change-setting-value (row)
  "Ask for and set a new value for the setting in ROW.
As the setting describes itself: a toggle or a choice is chosen from its
options; a folder is read as a file name; anything else is typed, and the
core reads and checks it, asking again with why when it refuses."
  (let* ((setting (plist-get row :setting))
         (key (plist-get setting :key))
         (now (plist-get setting :value))
         (options (mapcar (lambda (o) (cons (plist-get o :title) (plist-get o :id)))
                          (append (plist-get setting :options) nil)))
         (prompt (lumenna--prompt (plist-get setting :hint) (plist-get setting :title))))
    (cond (options
           (let ((value (cdr (assoc (completing-read (lumenna--with-default prompt (car (rassoc now options)))
                                                     options nil t nil nil (car (rassoc now options)))
                                    options))))
             (unless (equal value now) (lumenna--set key value))))
          ((equal (plist-get setting :kind) "folder")
           (let ((value (expand-file-name (read-directory-name prompt now))))
             (unless (equal value now) (lumenna--set key value))))
          (t
           ;; Applied when entered, never as it is typed; refused, asked again with why
           ;; and the line as it was typed.
           (let ((typed now) wrong done)
             (while (not done)
               (let ((value (read-string (if wrong (lumenna--prompt wrong prompt) prompt) typed)))
                 (if (equal value now)
                     (setq done t)
                   (lumenna--set key value)
                   (if-let* ((problem (lumenna--setting-problem key)))
                       (setq wrong problem typed value)
                     (setq done t))))))))))

;;;; Devices and syncing

(define-derived-mode lumenna-devices-mode lumenna-list-mode "Lumenna Devices"
  "Paired devices and how syncing with each last went.  RET renames one.

\\{lumenna-devices-mode-map}"
  (setq-local lumenna--held :devices_held)
  (setq-local lumenna--draw #'lumenna--device-rows)
  (setq-local lumenna--activate (lambda (_row) (lumenna-act-rename))))

(defun lumenna--device-rows ()
  "How syncing goes, then each paired device in the app's words."
  (let* ((devices (plist-get lumenna--view :devices))
         (list (plist-get devices :list))
         (shown (append (plist-get list :devices) nil))
         (lines (append (plist-get lumenna--lines :devices) nil)))
    ;; With no devices, the line under the heading says so, in the device list's words.
    (setq-local lumenna--empty (plist-get list :empty))
    (cons (lumenna--heading (if shown (plist-get devices :status) "Devices and sync")
                            (plist-get (plist-get devices :pairing) :problem))
          (cl-mapcar (lambda (device line)
                       (list :key (plist-get device :node_id) :text (plist-get line :text)
                             :device device :actions (plist-get device :actions)))
                     shown lines))))

;;;###autoload
(defun lumenna-devices ()
  "Show the paired devices, and how syncing with each is going."
  (interactive)
  (lumenna--show-screen "*Lumenna: Devices*" #'lumenna-devices-mode (list (list :Devices "Show"))))

(defmacro lumenna--on-devices (&rest body)
  "Run BODY in the Devices buffer, opened if it is not."
  (declare (indent 0))
  `(with-current-buffer (or (get-buffer "*Lumenna: Devices*")
                            (save-window-excursion (lumenna-devices)))
     ,@body))

;;;###autoload
(defun lumenna-sync-now ()
  "Sync with every paired device now."
  (interactive)
  (lumenna--on-devices
    (lumenna-send (list :Devices "SyncNow"))
    (lumenna--follow)))

;;;; Pairing

(defun lumenna--pairing-words ()
  "Pairing's sentences and buttons, the core's, as the Devices page has them."
  (lumenna--on-devices (plist-get (plist-get lumenna--view :devices) :words)))

(defun lumenna--pairing ()
  "The pairing this buffer's screen shows, if one is open."
  (plist-get (plist-get lumenna--view :devices) :pairing))

(defun lumenna--pair (event)
  "Send EVENT, something done while pairing."
  (lumenna-send (list :Devices (list :Pair event)))
  (lumenna--follow t))

(defun lumenna--read-code ()
  "The other device's code, as typed or yanked into the minibuffer.
Nothing is taken from the kill ring unasked, as in every other client."
  (let* ((words (lumenna--pairing-words))
         (typed (string-trim (read-string (lumenna--prompt "" (plist-get words :their_code))))))
    (if (string-empty-p typed)
        (user-error "%s" (string-remove-suffix "." (plist-get words :need_code)))
      typed)))

;;;###autoload
(defun lumenna-pair (&optional code)
  "Pair this device with another of yours.
Start pairing on both: on one network they find each other.  Otherwise type
on one the CODE the other shows; with a prefix argument, this asks for it.
Both show three words; say yes only if they are the same on both.  A code
given while this one waits gives up the wait and joins with it."
  (interactive (list (and current-prefix-arg (lumenna--read-code))))
  (lumenna--on-devices
    (unless (lumenna--pairing) (lumenna--pair (list :Open (system-name))))
    (if code
        (progn (lumenna--pair (list :Code (replace-regexp-in-string "[[:space:]]" "" code)))
               (lumenna--pair "Join"))
      (lumenna--pair "Wait"))))

(defun lumenna-pair-copy-code ()
  "Copy the code this device shows while it waits, to paste on the other device.
Only when asked: the clipboard is the person's own."
  (interactive)
  (lumenna--on-devices
    (let ((code (plist-get (lumenna--pairing) :mine)))
      (unless code (user-error "No code is shown; this device is not waiting to be found"))
      (kill-new code)
      (message "%s" (string-remove-suffix "." (plist-get (lumenna--pairing-words) :copied))))))

(defun lumenna-pair-cancel ()
  "Give up the pairing under way."
  (interactive)
  (lumenna--on-devices
    (unless (lumenna--pairing) (user-error "No pairing is under way"))
    (lumenna--pair "Cancel")))

(defvar lumenna--words-asked nil
  "The words last asked about, so a turn that shows them again does not ask twice.")

(defvar lumenna--code-shown nil
  "The code last shown, said once.")

(defun lumenna--pairing-turned (turn)
  "Say this device's code once it shows one, and ask whether the words match once
they come: a pairing's steps arrive by themselves, in TURN."
  (let* ((devices (plist-get (plist-get turn :view) :devices))
         (pairing (plist-get devices :pairing))
         (screen (plist-get turn :screen)))
    (when-let* ((code (plist-get pairing :mine)))
      (unless (equal code lumenna--code-shown)
        (setq lumenna--code-shown code)
        (let ((words (plist-get devices :words)))
          (message "%s %s: %s" (plist-get words :waiting) (plist-get words :my_code) code))))
    (let ((shown (append (plist-get pairing :words) nil)))
      (when (and shown (not (equal shown lumenna--words-asked)))
        (setq lumenna--words-asked shown)
        ;; Asked after the process filter has returned: a question from inside it
        ;; would hold up every other message.
        (run-at-time 0 nil #'lumenna--ask-words screen shown (plist-get devices :words))))))

(add-hook 'lumenna-turned-functions #'lumenna--pairing-turned)

(defun lumenna--ask-words (screen shown words)
  "Ask whether SHOWN, the three words, match the other device's; tell SCREEN.
WORDS are pairing's, the core's."
  (let* ((matched (yes-or-no-p
                   (lumenna--prompt (format "%s %s" (plist-get words :match_message) (string-join shown ", "))
                                    (plist-get words :match_title)))))
    (lumenna-send (list :Devices (list :Pair (list :Confirm (if matched t :json-false)))) screen)))

;;;; Backups, export and import

(defun lumenna-back-up-now ()
  "Back up the whole store now, history included."
  (interactive)
  (lumenna-say (lumenna-call "backup")))

(defun lumenna-restore-backup (file)
  "Merge backup FILE into the store.  Nothing here is lost; it only adds."
  (interactive (list (read-file-name "Restore backup: " nil nil t)))
  (lumenna-say (lumenna-call "restore" :file (expand-file-name file)))
  (lumenna--tick))

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
  (lumenna-say (lumenna-call "import" :file (expand-file-name file)))
  (lumenna--tick))

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
