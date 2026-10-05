;;; lumenna.el --- Tasks and a day planner, for Emacs -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Elijah Massey
;; SPDX-License-Identifier: MPL-2.0
;; Author: Elijah Massey
;; Version: 0.1.0
;; Package-Requires: ((emacs "29.1"))
;; Keywords: calendar, outlines, convenience
;; URL: https://github.com/emassey0135/lumenna

;;; Commentary:

;; Lumenna's Emacs client: the same tasks, projects, labels, filters, blocks
;; and devices as every other Lumenna app, in ordinary Emacs buffers.
;;
;; It speaks `lum rpc', Lumenna's command surface over JSON-RPC, through the
;; built-in `jsonrpc.el': the sync daemon's socket when one answers, and
;; otherwise a `lum rpc' of its own.  Nothing here parses a date, computes a
;; state or decides what completing a task does to its subtasks -- that is all
;; the core's, and an Emacs that reimplemented any of it would drift from the
;; phone.
;;
;; Every screen is a read-only buffer, one item per line, said in words: the
;; line is what any speech system reads, so Emacsvox, Emacspeak and speechd-el
;; all work with nothing more.  Faces mark done, overdue and the like, and
;; `lumenna-voice' maps them to voices where Emacspeak or Emacsvox are loaded;
;; `lumenna-emacsvox' adds semantic facts for Emacsvox's aural presentation.
;;
;; Start with M-x lumenna.  In any Lumenna buffer, ? lists every key it has,
;; and RET on one runs it.

;;; Code:

(require 'cl-lib)
(require 'derived)
(require 'easymenu)
(require 'jsonrpc)
(require 'outline)
(require 'subr-x)

(defgroup lumenna nil
  "Tasks and a day planner."
  :group 'applications
  :prefix "lumenna-")

(defcustom lumenna-lum-program "lum"
  "The `lum' program, by name on variable `exec-path' or by its full path."
  :type 'string)

(defcustom lumenna-profile nil
  "The profile directory, or nil for the one `lum' uses by default.
Like `lum', the environment variable LUMENNA_PROFILE is honoured when this
is nil."
  :type '(choice (const :tag "The default" nil) directory))

(defcustom lumenna-announce-function #'lumenna--message
  "How a result is said: called with the announcement and a list of notices."
  :type 'function)

(defvar lumenna-row-inserted-functions nil
  "Called with START, END and the ROW plist after each row is drawn.
`lumenna-emacsvox' uses this to attach semantic facts to the line.")

(defvar lumenna-changed-functions nil
  "Called with the RPC METHOD and its RESULT after a change succeeds.
`lumenna-voice' and `lumenna-emacsvox' use this for sounds.")

;;;; Faces

(defface lumenna-heading '((t :inherit bold))
  "The first line of a Lumenna buffer: what it shows, and how many.")

(defface lumenna-done '((t :inherit shadow :strike-through t))
  "A finished task.")

(defface lumenna-overdue '((t :inherit warning))
  "An overdue task.  The word \"overdue\" is always beside it.")

(defface lumenna-quiet '((t :inherit shadow))
  "Free time, an archived project, a cancelled day.")

(defface lumenna-now '((t :inherit bold))
  "The line for now in the day.")

(defface lumenna-running '((t :inherit success))
  "A sitting whose timer is running.")

;;;; The connection

(defconst lumenna-contract 1
  "The JSON shapes this client was written against.")

(defvar lumenna--connection nil
  "The open `jsonrpc-process-connection', or nil.")

(defun lumenna-profile-directory ()
  "The profile directory, by the rule `lum' itself uses."
  (or lumenna-profile
      (let ((explicit (getenv "LUMENNA_PROFILE")))
        (and explicit (not (string-empty-p explicit)) explicit))
      (pcase system-type
        ('darwin (expand-file-name "~/Library/Application Support/lumenna"))
        ('windows-nt (expand-file-name "lumenna/data" (getenv "LOCALAPPDATA")))
        (_ (let ((data (getenv "XDG_DATA_HOME")))
             (expand-file-name
              "lumenna"
              ;; As the `directories' crate does, a relative one is ignored.
              (if (and data (file-name-absolute-p data)) data "~/.local/share")))))))

(defun lumenna--socket ()
  "The sync daemon's socket, when there is a file for it."
  (unless (eq system-type 'windows-nt)
    (let ((socket (expand-file-name "lumenna.sock" (lumenna-profile-directory))))
      (and (file-exists-p socket) socket))))

(defun lumenna--open-process ()
  "A process speaking the surface.
The daemon's socket if it answers, else a `lum rpc' of its own."
  (or (when-let* ((socket (lumenna--socket)))
        ;; A socket left by a daemon that died does not answer; then spawn.
        (ignore-errors
          (make-network-process
           :name "lumenna" :family 'local :service socket
           :coding 'binary :noquery t)))
      (let ((program (executable-find lumenna-lum-program)))
        (unless program
          (user-error "Cannot find `%s'; install Lumenna or set `lumenna-lum-program'"
                      lumenna-lum-program))
        (make-process
         :name "lumenna"
         :command (append (list program)
                          (and lumenna-profile (list "--profile" (expand-file-name lumenna-profile)))
                          (list "rpc"))
         :connection-type 'pipe
         :coding 'binary
         :noquery t
         :stderr (get-buffer-create " *lumenna rpc stderr*")))))

(defun lumenna--connection ()
  "The connection, opened and checked if it is not already."
  (unless (and lumenna--connection (jsonrpc-running-p lumenna--connection))
    (let ((connection (make-instance 'jsonrpc-process-connection
                                     :name "lumenna"
                                     :process (lumenna--open-process)
                                     :notification-dispatcher #'lumenna--notified)))
      (let ((server (jsonrpc-request connection 'initialize nil)))
        (unless (equal (plist-get server :contract) lumenna-contract)
          (jsonrpc-shutdown connection)
          (user-error "This client reads version %s of Lumenna's data and `lum' speaks version %s; update whichever is older"
                      lumenna-contract (plist-get server :contract))))
      (setq lumenna--connection connection)))
  lumenna--connection)

(defun lumenna-disconnect ()
  "Close the connection to Lumenna, if one is open."
  (interactive)
  (when lumenna--connection
    (ignore-errors (jsonrpc-shutdown lumenna--connection))
    (setq lumenna--connection nil)))

(defun lumenna--error-message (err)
  "The core's sentence from a `jsonrpc-error' ERR."
  (or (cdr (assq 'jsonrpc-error-message (cdr err))) (format "%s" err)))

(defun lumenna--params (params)
  "PARAMS, a plist, with nil values left out.
The server reads a missing key as absent, where a null would sometimes be
read as a value."
  (let (out)
    (while params
      (unless (null (cadr params))
        (setq out (append out (list (car params) (cadr params)))))
      (setq params (cddr params)))
    (or out (make-hash-table))))

(defun lumenna-call (method &rest params)
  "Call METHOD with PARAMS, a plist, and return the result.
A refusal is a `user-error' carrying the core's own sentence."
  (condition-case err
      (jsonrpc-request (lumenna--connection) (intern method) (lumenna--params params)
                       :timeout 30)
    (jsonrpc-error (user-error "%s" (lumenna--error-message err)))))

(defun lumenna-write (method &rest params)
  "Call METHOD with PARAMS, which changes the store; redraw, and say it.
Returns the result."
  (let ((result (apply #'lumenna-call method params)))
    (lumenna-refresh-all)
    (run-hook-with-args 'lumenna-changed-functions method result)
    (lumenna-say result)
    result))

(defun lumenna-say (result)
  "Say RESULT's announcement and notices."
  (funcall lumenna-announce-function
           (or (plist-get result :announcement) "")
           (append (plist-get result :notices) nil)))

(defun lumenna--message (announcement notices)
  "Show ANNOUNCEMENT and NOTICES in the echo area.
Every speech system reads the echo area."
  (let ((text (string-join (seq-remove #'string-empty-p (cons announcement notices)) ". ")))
    (unless (string-empty-p text)
      (message "%s" text))))

(defun lumenna--true (value)
  "Whether a JSON VALUE is true; false arrives as `:json-false'."
  (eq value t))

(defun lumenna--notified (_connection method params)
  "Handle notification METHOD with PARAMS from the server."
  (pcase method
    ('lumenna/changed (run-at-time 0 nil #'lumenna-refresh-all))
    ('lumenna/pairing
     ;; Asked after the process filter has returned: a question from inside it
     ;; would hold up every other message.
     (run-at-time 0 nil #'lumenna-pairing-notified params))))

;;;; Rows and the buffers that list them

(defun lumenna-describe (row)
  "ROW as one line of words: the title, done, the value, then the states.
\"ready\" is true of almost every task, so saying it everywhere would bury
the states that mean something."
  (string-join
   (delq nil (append (list (plist-get row :title)
                           (and (lumenna--true (plist-get row :checked)) "done")
                           (plist-get row :value))
                     (seq-remove (lambda (state) (equal state "ready"))
                                 (append (plist-get row :state) nil))))
   ", "))

(defun lumenna--row-face (row)
  "The face for ROW, from its states."
  (let ((states (append (plist-get row :state) nil)))
    (cond ((lumenna--true (plist-get row :checked)) 'lumenna-done)
          ((member "overdue" states) 'lumenna-overdue)
          ((member "archived" states) 'lumenna-quiet)
          (t (plist-get row :face)))))

(defvar-local lumenna--source nil
  "A function of no arguments returning (HEADING . ROWS) for this buffer.")

(defvar-local lumenna--describe #'lumenna-describe
  "A function turning one row into its line.")

(defvar-local lumenna--activate nil
  "What RET does: a function of the row at point.")

(defun lumenna--insert-row (row)
  "Insert ROW as one line, indented by its depth, carrying the row."
  (let* ((depth (or (plist-get row :depth) 0))
         (start (point)))
    (insert (make-string (* 2 depth) ?\s) (funcall lumenna--describe row))
    (let ((face (lumenna--row-face row)))
      (when face (put-text-property start (point) 'face face)))
    (add-text-properties start (point) (list 'lumenna-row row 'lumenna-level (+ depth 2)))
    (run-hook-with-args 'lumenna-row-inserted-functions start (point) row)
    (insert "\n")))

(defun lumenna-row ()
  "The row at point, or a `user-error'."
  (or (get-text-property (line-beginning-position) 'lumenna-row)
      (user-error "No item on this line")))

(defun lumenna--row-id (row)
  "What tells ROW apart from the rest of its list."
  (or (plist-get row :id) (plist-get row :key)))

(defun lumenna-refresh ()
  "Read this buffer's list again, keeping point on the same item.
If it is gone, point stays on the line that took its place."
  (interactive)
  (when lumenna--source
    (let* ((inhibit-read-only t)
           (here (get-text-property (line-beginning-position) 'lumenna-row))
           (id (and here (lumenna--row-id here)))
           (line (line-number-at-pos))
           (listing (funcall lumenna--source)))
      (erase-buffer)
      (let ((start (point)))
        (insert (car listing))
        (add-text-properties start (point) '(face lumenna-heading lumenna-level 1))
        (insert "\n"))
      (mapc #'lumenna--insert-row (cdr listing))
      (goto-char (point-min))
      (if-let* ((found (and id (lumenna--find id))))
          (goto-char found)
        (forward-line (1- (min line (count-lines (point-min) (point-max)))))
        (when (and (bobp) (cdr listing)) (forward-line 1))))))

(defun lumenna--find (id)
  "The start of the line whose row is ID, if there is one."
  (save-excursion
    (goto-char (point-min))
    (let (found)
      (while (and (not found) (not (eobp)))
        (let ((row (get-text-property (point) 'lumenna-row)))
          (when (and row (equal (lumenna--row-id row) id)) (setq found (point))))
        (forward-line 1))
      found)))

(defun lumenna-refresh-all ()
  "Redraw every Lumenna buffer, as the store has moved."
  (dolist (buffer (buffer-list))
    (with-current-buffer buffer
      (when (derived-mode-p 'lumenna-list-mode)
        (condition-case err
            (lumenna-refresh)
          (user-error (message "%s" (cadr err))))))))

(defun lumenna-activate ()
  "Do the main thing for the item at point."
  (interactive)
  (if lumenna--activate
      (funcall lumenna--activate (lumenna-row))
    (user-error "Nothing to do here")))

(defun lumenna-toggle ()
  "Fold or unfold the item at point, if anything sits under it."
  (interactive)
  (outline-toggle-children))

(defun lumenna-undo ()
  "Undo the last change this device made to the store, wherever it was made."
  (interactive)
  (lumenna-write "undo"))

(defun lumenna-redo ()
  "Redo the change most recently undone."
  (interactive)
  (lumenna-write "redo"))

(defun lumenna--outline-level ()
  "The outline level of the line at point: the item's depth, below the heading."
  (or (get-text-property (line-beginning-position) 'lumenna-level) 1))

(defvar lumenna-command-map (make-sparse-keymap)
  "Lumenna's commands, from anywhere.
Bind it to a prefix, as (keymap-global-set \"C-c l\" lumenna-command-map);
its ? lists the rest.")

(defmacro lumenna-define-keys (owner &rest groups)
  "Bind GROUPS in OWNER's keymap, and keep them for its help.
OWNER is a mode, whose map is OWNER-map, or a keymap variable.  Each group
is (HEADING (KEY DESCRIPTION COMMAND)...).  The same definition does both,
so the help cannot name a key the buffer lacks."
  (declare (indent 1))
  `(lumenna--define-keys ',owner ',groups))

(defun lumenna--define-keys (owner groups)
  "Bind GROUPS in OWNER's keymap, keep them for its help, and mark the mode.
A Lumenna command bound only in modes is marked as theirs, as
\(interactive nil MODE) would, so \[execute-extended-command] can leave it out
elsewhere; one the global map also binds works anywhere."
  (let* ((mode (and (boundp (derived-mode-map-name owner)) owner))
         (map (symbol-value (if mode (derived-mode-map-name owner) owner))))
    (dolist (group groups)
      (dolist (binding (cdr group))
        (let ((command (nth 2 binding)))
          (keymap-set map (car binding) command)
          (cond ((not mode) (function-put command 'command-modes nil))
                ((and (string-prefix-p "lumenna-" (symbol-name command))
                      (not (where-is-internal command lumenna-command-map t)))
                 (function-put command 'command-modes
                               (cl-adjoin mode (function-get command 'command-modes)))))))))
  (put owner 'lumenna-keys groups)
  (lumenna--define-menu owner))

(defun lumenna--key-groups (owner)
  "OWNER's key groups, then those of the modes it derives from.
A key is listed once, under the nearest mode that binds it, as it acts."
  (let (seen groups)
    (while owner
      (dolist (group (get owner 'lumenna-keys))
        (let ((bindings (seq-remove (lambda (binding) (member (car binding) seen)) (cdr group))))
          (setq seen (append (mapcar #'car bindings) seen))
          (when bindings (push (cons (car group) bindings) groups))))
      (setq owner (get owner 'derived-mode-parent)))
    (nreverse groups)))

(defun lumenna--menu (owner)
  "A menu of OWNER's keys, grouped as its help groups them.
Where two keys run one command, the menu lists it once; the menu shows
whichever key it is reached by."
  (cons "Lumenna"
        (mapcar (lambda (group)
                  (let (seen)
                    (cons (car group)
                          (delq nil (mapcar (lambda (binding)
                                              (unless (memq (nth 2 binding) seen)
                                                (push (nth 2 binding) seen)
                                                (vector (nth 1 binding) (nth 2 binding))))
                                            (cdr group))))))
                (lumenna--key-groups owner))))

(defun lumenna--define-menu (owner)
  "Give OWNER's keys a menu: a mode's in its own map, the global ones under Tools.
The same definition makes the keys, the help and the menu, so a person can
use whichever they prefer and find the same commands."
  (if (boundp (derived-mode-map-name owner))
      (easy-menu-define nil (symbol-value (derived-mode-map-name owner))
        "Lumenna's commands here." (lumenna--menu owner))
    (define-key-after (lookup-key global-map [menu-bar tools]) [lumenna]
      (cons "Lumenna" (easy-menu-create-menu "Lumenna" (cdr (lumenna--menu owner)))))))

(define-derived-mode lumenna-list-mode special-mode "Lumenna"
  "A Lumenna list: one item per line, said in words.
Subtasks sit under their task as an outline, so TAB folds them.

\\{lumenna-list-mode-map}"
  (setq-local truncate-lines nil)
  (setq-local revert-buffer-function (lambda (&rest _) (lumenna-refresh)))
  ;; Every line is an outline heading, its level the item's depth: folding,
  ;; and Emacsvox's and Emacspeak's outline support, come from Emacs itself.
  (setq-local outline-regexp "[^\n]")
  (setq-local outline-level #'lumenna--outline-level)
  (setq-local imenu-create-index-function #'lumenna--imenu-index)
  (outline-minor-mode 1))

(lumenna-define-keys lumenna-list-mode
  ("Every list"
   ("n" "Next line" next-line)
   ("p" "Previous line" previous-line)
   ("RET" "Do the main thing for this line" lumenna-activate)
   ("TAB" "Fold or unfold what sits under this line" lumenna-toggle)
   ("g" "Read the list again" lumenna-refresh)
   ("u" "Undo" lumenna-undo)
   ("y" "Redo" lumenna-redo)
   ("?" "These keys" lumenna-help)
   ("h" "These keys" lumenna-help)
   ("L" "Lumenna's places" lumenna)
   ("q" "Leave this list" quit-window)))

;; Emacs's own undo has nothing to undo in a read-only list; what is meant
;; there is the store's.
(keymap-set lumenna-list-mode-map "<remap> <undo>" #'lumenna-undo)
(keymap-set lumenna-list-mode-map "<remap> <undo-redo>" #'lumenna-redo)

(defun lumenna--imenu-index ()
  "Every item in this list, by its line, for `imenu' to go to."
  (let (index)
    (save-excursion
      (goto-char (point-min))
      (while (not (eobp))
        (when (get-text-property (point) 'lumenna-row)
          (push (cons (string-trim (buffer-substring-no-properties (point) (line-end-position)))
                      (point))
                index))
        (forward-line 1)))
    (nreverse index)))

(defun lumenna--show-list (name mode source &rest settings)
  "Show buffer NAME in MODE, listing SOURCE; SETTINGS are buffer-local pairs."
  (let ((buffer (get-buffer-create name)))
    (with-current-buffer buffer
      (unless (derived-mode-p mode) (funcall mode))
      (setq lumenna--source source)
      (while settings
        (set (make-local-variable (car settings)) (cadr settings))
        (setq settings (cddr settings)))
      (lumenna-refresh))
    (pop-to-buffer-same-window buffer)
    buffer))

;;;; Typing: completion and quick add

(defun lumenna--byte-offset (text chars)
  "CHARS characters into TEXT, as the UTF-8 byte offset the core counts in."
  (string-bytes (substring text 0 (min chars (length text)))))

(defun lumenna--char-offset (text bytes)
  "BYTES into TEXT's UTF-8, as a character offset, never inside a character."
  (let ((encoded (encode-coding-string text 'utf-8)))
    (length (decode-coding-string (substring encoded 0 (min bytes (length encoded))) 'utf-8))))

(defun lumenna--completion (syntax)
  "A `completion-at-point' function asking the core what could go at point.
SYNTAX is the language of the line, \"quick-add\" or \"filter\"."
  (lambda ()
    (let* ((start (minibuffer-prompt-end))
           (text (minibuffer-contents-no-properties))
           (cursor (- (point) start))
           (found (ignore-errors
                    (lumenna-call "complete" :text text :syntax syntax
                                  :cursor (lumenna--byte-offset text cursor))))
           (candidates (and found (append (plist-get found :candidates) nil))))
      (when candidates
        (let ((labels (mapcar (lambda (c) (cons (plist-get c :text) (plist-get c :label))) candidates)))
          (list (+ start (lumenna--char-offset text (plist-get found :start)))
                (+ start (lumenna--char-offset text (plist-get found :end)))
                (mapcar #'car labels)
                :exclusive 'no
                ;; "project Work", not "#Work": the sigil is punctuation a
                ;; speech system may skip.
                :annotation-function
                (lambda (candidate) (concat "  " (cdr (assoc candidate labels))))))))))

(defvar-keymap lumenna-minibuffer-map
  :doc "The minibuffer for a quick-add line or a filter.
TAB completes; C-c C-r says how the line is understood so far."
  :parent minibuffer-local-map
  "TAB" #'completion-at-point
  "C-c C-r" #'lumenna-read-back)

(defvar-local lumenna--syntax nil
  "The language the minibuffer is reading, \"quick-add\" or \"filter\".")

(declare-function lumenna--task-listing "lumenna-tasks" (query title))

(defun lumenna-read-back ()
  "Say how the line typed so far is understood, before it is entered.
What the other apps show under the field as it is typed: a task's date,
priority and labels, or a filter's meaning and how many tasks it matches."
  (interactive nil minibuffer-mode)
  (let ((text (minibuffer-contents-no-properties)))
    (funcall lumenna-announce-function
             (cond ((string-blank-p text) "Nothing typed yet")
                   ((equal lumenna--syntax "quick-add")
                    (let ((preview (lumenna-call "preview" :text text)))
                      (string-join (cons (plist-get preview :announcement)
                                         (mapcar (lambda (d) (plist-get d :message))
                                                 (append (plist-get preview :diagnostics) nil)))
                                   ". ")))
                   (t (car (lumenna--task-listing text "Filter"))))
             nil)))

(defvar lumenna-add-history nil "Quick-add lines typed before.")
(defvar lumenna-filter-history nil "Filters typed before.")

(defun lumenna-read-line (prompt syntax &optional initial history)
  "Read a line in SYNTAX, \"quick-add\" or \"filter\", asking PROMPT.
The core completes it and reads it back.  INITIAL starts the line; HISTORY
is the history variable."
  (minibuffer-with-setup-hook
      (lambda ()
        (setq lumenna--syntax syntax)
        (add-hook 'completion-at-point-functions (lumenna--completion syntax) nil t))
    (read-from-minibuffer prompt initial lumenna-minibuffer-map nil history)))

;;;###autoload
(defun lumenna-add (&optional prefix)
  "Add a task, written the way you would say it.
Such as \"call the bank tomorrow at 3pm p1 #Home @calls 15m\".  TAB completes
a project or label, and \\<lumenna-minibuffer-map>\\[lumenna-read-back] says how the line is understood
so far.  The line is checked first: an unknown project is said
and nothing is added; an unknown label becomes a new label.  PREFIX starts
the line, as a project's list does with its own name."
  (interactive)
  (let ((text (lumenna-read-line "Add a task: " "quick-add" prefix 'lumenna-add-history)))
    (when (string-blank-p text) (user-error "Nothing to add"))
    (let ((preview (lumenna-call "preview" :text text)))
      (when (lumenna--true (plist-get preview :has_errors))
        (user-error "%s" (string-join
                          (delq nil (mapcar (lambda (d) (and (equal (plist-get d :severity) "error")
                                                             (plist-get d :message)))
                                            (append (plist-get preview :diagnostics) nil)))
                          "; "))))
    (lumenna-write "task.add" :text text)))

;;;###autoload
(defun lumenna-search (query)
  "List the tasks matching QUERY, a filter such as \"#Work & overdue\".
\"search:\" followed by words looks through titles and notes.  TAB completes."
  (interactive (list (lumenna-read-line "Filter, or search: and words: " "filter" nil 'lumenna-filter-history)))
  (lumenna-tasks query "Query"))

;;;; The places, and the menu of everything

(defconst lumenna--places
  '(("Today" . lumenna-today) ("Tasks" . lumenna-tasks) ("Projects" . lumenna-projects)
    ("Labels" . lumenna-labels) ("Saved filters" . lumenna-filters) ("Blocks" . lumenna-blocks)
    ("Trash" . lumenna-trash) ("Devices and sync" . lumenna-devices) ("Settings" . lumenna-settings))
  "Lumenna's places, as the main buffer lists them.")

(define-derived-mode lumenna-home-mode lumenna-list-mode "Lumenna"
  "Lumenna's places: RET opens one.

\\{lumenna-home-mode-map}")

;;;###autoload
(defun lumenna ()
  "Open Lumenna: its places, one per line.  RET opens one; ? shows every command."
  (interactive)
  (lumenna--show-list
   "*Lumenna*" #'lumenna-home-mode
   (lambda ()
     (cons "Lumenna"
           (mapcar (lambda (place) (list :key (car place) :title (car place) :command (cdr place)))
                   lumenna--places)))
   'lumenna--activate (lambda (row) (call-interactively (plist-get row :command)))))

;;;; Keys, listed

;; ? lists a buffer's keys in an ordinary buffer rather than a pop-up menu: one
;; line per key, read the way every other line is, by any screen reader.  A
;; transient menu needs its screen reader to follow transient's own window, and
;; Emacsvox's support for it falls silent under Emacs 31's transient.

(define-derived-mode lumenna-help-mode lumenna-list-mode "Lumenna Keys"
  "The keys of a Lumenna buffer, one per line.  RET runs one there.

\{lumenna-help-mode-map}")

(defvar-local lumenna--help-origin nil
  "The buffer whose keys this lists, where RET runs them.")

(defun lumenna--show-help (owner title)
  "List OWNER's keys under TITLE, to be run in the current buffer."
  (let ((origin (current-buffer))
        (groups (lumenna--key-groups owner)))
    (lumenna--show-list
     "*Lumenna keys*" #'lumenna-help-mode
     (lambda ()
       (cons (format "Keys in %s. RET runs one there, q goes back" title)
             (mapcan (lambda (group)
                       (cons (list :role "heading" :key (car group) :title (car group)
                                   :face 'lumenna-heading)
                             (mapcar (lambda (binding)
                                       (list :role "key" :key (car binding) :depth 1
                                             :title (format "%s: %s" (car binding) (nth 1 binding))
                                             :command (nth 2 binding)))
                                     (cdr group))))
                     groups)))
     'lumenna--help-origin origin
     'lumenna--activate #'lumenna--help-run)))

(defun lumenna--help-run (row)
  "Go back to the buffer the keys are for, and run ROW's command there."
  (let ((command (or (plist-get row :command) (user-error "A heading; its keys are under it")))
        (origin lumenna--help-origin))
    (quit-window)
    (when (buffer-live-p origin) (pop-to-buffer-same-window origin))
    (call-interactively command)))

(defun lumenna-help ()
  "List every key this buffer has, with what it does.  RET on one runs it."
  (interactive)
  (if (derived-mode-p 'lumenna-list-mode)
      (lumenna--show-help major-mode (string-trim (buffer-name) "\\*" "\\*"))
    (lumenna-dispatch)))

(lumenna-define-keys lumenna-command-map
  ("Places"
   ("t" "Today" lumenna-today)
   ("k" "Tasks" lumenna-tasks)
   ("p" "Projects" lumenna-projects)
   ("l" "Labels" lumenna-labels)
   ("f" "Saved filters" lumenna-filters)
   ("b" "Blocks" lumenna-blocks)
   ("x" "Trash" lumenna-trash)
   ("L" "All of Lumenna's places" lumenna))
  ("Do"
   ("a" "Add a task" lumenna-add)
   ("/" "Search or filter" lumenna-search)
   ("u" "Undo" lumenna-undo)
   ("y" "Redo" lumenna-redo)
   ("n" "Sync now" lumenna-sync-now))
  ("Settings"
   ("d" "Devices and sync" lumenna-devices)
   ("s" "Settings" lumenna-settings))
  ("Help"
   ("?" "These keys" lumenna-dispatch)))

;;;###autoload
(defun lumenna-dispatch ()
  "List everything Lumenna does from anywhere; RET on one runs it."
  (interactive)
  (lumenna--show-help 'lumenna-command-map "Lumenna, from anywhere"))

(provide 'lumenna)

(require 'lumenna-tasks)
(require 'lumenna-day)
(require 'lumenna-organise)
(require 'lumenna-settings)
(require 'lumenna-voice)
(with-eval-after-load 'emacsvox-aural-submission
  (require 'lumenna-emacsvox))

;;; lumenna.el ends here
