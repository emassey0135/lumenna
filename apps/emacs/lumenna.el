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
;; It draws Lumenna's app, which every other client draws too: the app runs in
;; `lum rpc' (or in the sync daemon or a desktop app, over its socket), and
;; each Lumenna buffer is one of its screens.  A buffer sends what the person
;; did, and the app answers with the screen as it now is -- its rows worded,
;; the row in hand, the question an action asks -- through the built-in
;; `jsonrpc.el'.  Nothing here decides which row is in hand after a change,
;; what an action asks, or what a list says: that is the app's, and an Emacs
;; that decided any of it would drift from the phone.
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
(require 'time)

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
  "How what the app says is said.
Called with its first sentence and a list of the rest."
  :type 'function)

(defvar lumenna-row-inserted-functions nil
  "Called with START, END and the row, after each row's line is inserted.")

(defvar lumenna-connected-hook nil
  "Run once a connection to the server is open and checked.")

(defvar lumenna-changed-functions nil
  "Called with the action and nil just before what an action did is said.
The action is `SUBJECT/KIND', such as \"task/mark_done\": what
`lumenna-voice' and `lumenna-emacsvox' key their sounds on.")

(defvar lumenna-turned-functions nil
  "Called with each turn, once its buffers are drawn: for what comes by itself,
such as a pairing's words to compare.")

(defvar lumenna-followed-hook nil
  "Run in a buffer once the app has done what the person asked of it there.
What the app opened by itself -- a proposal, a reminder list -- is opened
from here, in a buffer of its own.")

(defface lumenna-heading '((t :inherit bold))
  "A list's heading.")

(defface lumenna-done '((t :inherit shadow :strike-through t))
  "A finished task.")

(defface lumenna-overdue '((t :inherit warning))
  "A task past its due date.")

(defface lumenna-quiet '((t :inherit shadow))
  "Something worth less attention: free time, an archived project.")

(defface lumenna-now '((t :inherit bold))
  "The line marking now, in a day.")

(defface lumenna-running '((t :inherit success))
  "A sitting whose timer is running.")

(defface lumenna-priority-1 '((t :inherit error))
  "A task of priority 1, the highest.")

(defface lumenna-priority-2 '((t :inherit warning))
  "A task of priority 2.")

(defface lumenna-priority-3 '((t :inherit font-lock-keyword-face))
  "A task of priority 3.")

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
          ;; UTF-8, as Eglot opens its connections: Emacs 30's jsonrpc makes it binary
          ;; itself, but Emacs 29's reads what arrives as text, and on binary its JSON
          ;; parser met a raw byte's internal form (a 0xC1 lead) in the first non-ASCII
          ;; character, a reminder's dash.
          (make-network-process
           :name "lumenna" :family 'local :service socket
           :coding 'utf-8-emacs-unix :noquery t)))
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
         :coding 'utf-8-emacs-unix
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
      (setq lumenna--connection connection)
      (clrhash lumenna--screens)
      (setq lumenna--clock nil)
      (lumenna--follow-clock)
      ;; What pairing's sentences call this device, which finds others on its network.
      (jsonrpc-notify connection 'app.send (list :screen 0 :event (list :PairingAs (list :called "this computer" :nearby t))))
      (lumenna--start-ticking)
      (run-hooks 'lumenna-connected-hook)))
  lumenna--connection)

(defvar lumenna--clock nil
  "The clock the app was last told: `24' or `12'.
As `display-time-24hr-format' had it.")

(defun lumenna--follow-clock ()
  "Tell the app Emacs's clock, the mode line's, when it has changed."
  (let ((clock (if display-time-24hr-format '24 '12)))
    (unless (eq clock lumenna--clock)
      (setq lumenna--clock clock)
      (jsonrpc-notify lumenna--connection 'app.clock (list :twenty_four (if (eq clock '24) t :json-false))))))

(defun lumenna-disconnect ()
  "Close the connection to Lumenna, if one is open."
  (interactive)
  (lumenna--stop-ticking)
  (clrhash lumenna--screens)
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

(defun lumenna-say (result)
  "Say RESULT's announcement and notices: what a method that is not the app's said."
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
    ('lumenna/turn (lumenna--turned params))
    ;; Another process wrote: the app takes it in, and its turns redraw.
    ('lumenna/changed (run-at-time 0 nil #'lumenna--tick))))

;;;; The app's screens

;; Each Lumenna buffer draws one of the app's screens, its own place, row in
;; hand and question; a buffer opened from another -- a proposal from a day --
;; draws the same screen.  Screen 0 is the places' buffer, `*Lumenna*'.

(defvar lumenna--screens (make-hash-table)
  "The buffers drawing each screen open, by its number.")

(defvar lumenna--next-screen 1
  "The number the next screen takes.")

(defvar-local lumenna--screen nil
  "The app's screen this buffer draws.")

(defvar-local lumenna--view nil
  "What the app last showed on this buffer's screen.")

(defvar-local lumenna--lines nil
  "That view's lines, in the core's words.")

(defvar-local lumenna--draw nil
  "A function of no arguments giving this buffer's (HEADING . ROWS), from its view.")

(defvar-local lumenna--held nil
  "Which side list this buffer lists, as the view names its row in hand:
`:sidebar_held', `:reminders_held' or `:devices_held'.  Nil for the screen's
own list.")

(defvar-local lumenna--landed nil
  "The landing count point last went to the row in hand for.")

(defvar-local lumenna--empty nil
  "What this list says when it has nothing in it: the listing's own `empty'.")

(defvar-local lumenna--source nil
  "For a buffer that is not one of the app's, a function of no arguments
returning (HEADING . ROWS): a list of keys, the reminders due.")

(defun lumenna--buffers-of (screen)
  "The live buffers drawing SCREEN."
  (seq-filter #'buffer-live-p (gethash screen lumenna--screens)))

(defun lumenna--attach (screen)
  "Draw SCREEN in this buffer, beside any other buffer drawing it."
  (setq lumenna--screen screen)
  (puthash screen (append (lumenna--buffers-of screen) (list (current-buffer))) lumenna--screens)
  (add-hook 'kill-buffer-hook #'lumenna--detach nil t))

(defun lumenna--detach ()
  "This buffer no longer draws its screen; the last to go closes it."
  (when-let* ((screen lumenna--screen))
    (let ((left (remq (current-buffer) (lumenna--buffers-of screen))))
      (if left
          (puthash screen left lumenna--screens)
        (remhash screen lumenna--screens)
        (when (and (/= screen 0) lumenna--connection (jsonrpc-running-p lumenna--connection))
          (ignore-errors (jsonrpc-notify lumenna--connection 'app.close (list :screen screen))))))))

(defun lumenna-send (event &optional screen)
  "Send EVENT, something the person did, to SCREEN or this buffer's.
EVENT is the app's in its JSON shape: \"Undo\", (:Focus ID),
\(:Answer (:answer \"yes\")).  Its turns come back as the app makes them."
  (jsonrpc-notify (lumenna--connection) 'app.send
                  (list :screen (or screen lumenna--screen 0) :event event)))

(defun lumenna--settle ()
  "Wait until the app is still: everything sent done, every turn drawn."
  (let ((connection (lumenna--connection))
        (deadline (+ (float-time) 30)))
    (while (and (not (lumenna--true (jsonrpc-request connection 'app.still nil :timeout 30)))
                (< (float-time) deadline))
      (accept-process-output nil 0.01))))

(defvar lumenna--acting nil
  "The action whose sentence is still to come, as `SUBJECT/KIND'.")

(defvar lumenna--heard nil
  "While bound to a list, what turns say is kept there instead of said: a refusal.")

(defun lumenna--turned (turn)
  "Draw TURN in every buffer drawing its screen, then say what it says."
  (let ((screen (plist-get turn :screen)))
    (dolist (buffer (lumenna--buffers-of screen))
      (with-current-buffer buffer
        (setq lumenna--view (plist-get turn :view)
              lumenna--lines (plist-get turn :lines))
        (lumenna--redraw)))
    (dolist (file (append (plist-get turn :files) nil))
      ;; Where to keep it is asked after the process filter has returned.
      (run-at-time 0 nil #'lumenna--save-file file))
    (let ((said (append (plist-get turn :said) nil)))
      (when (and said lumenna--heard)
        (setcdr (last lumenna--heard) said)
        (setq said nil))
      (when said
        (when lumenna--acting
          (run-hook-with-args 'lumenna-changed-functions lumenna--acting nil)
          (setq lumenna--acting nil))
        (funcall lumenna-announce-function (car said) (cdr said))))
    (run-hook-with-args 'lumenna-turned-functions turn)))

(defun lumenna--save-file (file)
  "Save FILE, a backup or an export the app made, where the person says."
  (let ((name (read-file-name "Save as: " nil nil nil (plist-get file :name))))
    (let ((coding-system-for-write 'binary))
      (write-region (base64-decode-string (plist-get file :data)) nil name))
    (message "Saved %s" (abbreviate-file-name name))))

(defvar lumenna--ticking nil
  "The timer that takes in another process's writes and the minute.")

(defun lumenna--tick ()
  "Have the app take in what changed elsewhere, or a new minute."
  (when (and lumenna--connection (jsonrpc-running-p lumenna--connection))
    (jsonrpc-notify lumenna--connection 'app.tick (make-hash-table))
    (dolist (buffer (buffer-list))
      (with-current-buffer buffer
        (when (and lumenna--source (derived-mode-p 'lumenna-list-mode))
          (ignore-errors (lumenna-refresh)))))))

(defun lumenna--start-ticking ()
  "Tick every half minute, for now to move."
  (lumenna--stop-ticking)
  (setq lumenna--ticking (run-at-time 30 30 #'lumenna--tick)))

(defun lumenna--stop-ticking ()
  "Stop ticking."
  (when lumenna--ticking
    (cancel-timer lumenna--ticking)
    (setq lumenna--ticking nil)))

(defun lumenna--show-screen (name mode events &optional screen)
  "Show buffer NAME in MODE, drawing a screen sent EVENTS; return the buffer.
SCREEN is the screen to draw, else a new one, or the buffer's own if NAME
already draws one.  EVENTS are sent there first."
  (lumenna--connection)
  (lumenna--follow-clock)
  (let ((buffer (get-buffer-create name)))
    (with-current-buffer buffer
      (unless (derived-mode-p mode) (funcall mode))
      (unless lumenna--screen
        (lumenna--attach (or screen (prog1 lumenna--next-screen (cl-incf lumenna--next-screen)))))
      (dolist (event events) (lumenna-send event))
      (lumenna--settle)
      (unless lumenna--view
        ;; A screen the app had already drawn as it was: no turn comes for it.
        (let ((shown (jsonrpc-request (lumenna--connection) 'app.view (list :screen lumenna--screen))))
          (setq lumenna--view (plist-get shown :view) lumenna--lines (plist-get shown :lines))
          (lumenna--redraw))))
    (pop-to-buffer-same-window buffer)
    (set-buffer buffer)
    (lumenna--follow t)
    (set-buffer buffer)
    buffer))

(defun lumenna--home-buffer ()
  "The places' buffer, screen 0, made if it is not there yet, not shown."
  (or (get-buffer "*Lumenna*")
      (with-current-buffer (get-buffer-create "*Lumenna*")
        (lumenna--connection)
        (lumenna-home-mode)
        (lumenna--attach 0)
        (lumenna--settle)
        (let ((shown (jsonrpc-request (lumenna--connection) 'app.view (list :screen 0))))
          (setq lumenna--view (plist-get shown :view) lumenna--lines (plist-get shown :lines))
          (lumenna--redraw))
        (current-buffer))))

(defmacro lumenna--on-a-screen (&rest body)
  "Run BODY in this buffer if it draws a screen, else in the places' buffer."
  (declare (indent 0))
  `(if lumenna--screen
       (progn ,@body)
     (with-current-buffer (lumenna--home-buffer) ,@body)))

;;;; Rows and the buffers that list them

(defun lumenna--row-face (row)
  "The face for ROW, from its states."
  (let ((states (append (plist-get row :state) nil)))
    (cond ((lumenna--true (plist-get row :checked)) 'lumenna-done)
          ((member "overdue" states) 'lumenna-overdue)
          ((member "archived" states) 'lumenna-quiet)
          ;; The row says its priority in words too; the face and its voice only add to that.
          ((plist-get row :priority) (intern (format "lumenna-priority-%d" (plist-get row :priority))))
          (t (plist-get row :face)))))

(defun lumenna--heading (&rest parts)
  "A heading from PARTS that are not empty, as sentences."
  (string-join (seq-remove (lambda (p) (or (null p) (string-empty-p p))) parts) ". "))

(defun lumenna--swatch (swatch)
  "SWATCH, a colour of the core's palette, as a mark before a name.
Two spaces in its colour, then one: blank to speech and braille, which say
the colour's name in the line.  Nothing where the display has no colour."
  (if (and swatch (display-color-p))
      (concat (propertize "  " 'face (list :background
                                           (plist-get swatch (if (eq (frame-parameter nil 'background-mode) 'dark)
                                                                 :dark :light))))
              " ")
    ""))

(defun lumenna--insert-row (row)
  "Insert ROW as one line, indented by its depth, carrying the row."
  (let* ((depth (or (plist-get row :depth) 0))
         (start (point)))
    (insert (make-string (* 2 depth) ?\s)
            (lumenna--swatch (or (plist-get row :swatch) (plist-get (plist-get row :block) :swatch))))
    (let ((text (point)))
      (insert (or (plist-get row :text) (plist-get row :title)))
      (let ((face (lumenna--row-face row)))
        (when face (put-text-property text (point) 'face face))))
    (add-text-properties start (point) (list 'lumenna-row row 'lumenna-level (+ depth 2)))
    (run-hook-with-args 'lumenna-row-inserted-functions start (point) row)
    (insert "\n")))

(defun lumenna-row ()
  "The row at point, or a `user-error'."
  (or (get-text-property (line-beginning-position) 'lumenna-row)
      (user-error "No item on this line")))

(defun lumenna--row-here ()
  "The row on this line, or nil."
  (get-text-property (line-beginning-position) 'lumenna-row))

(defun lumenna--row-key (row)
  "What tells ROW apart from the rest of its list."
  (or (plist-get row :key) (plist-get row :id)))

(defun lumenna--find (key)
  "The start of the line whose row is KEY, if there is one."
  (when key
    (save-excursion
      (goto-char (point-min))
      (let (found)
        (while (and (not found) (not (eobp)))
          (let ((row (get-text-property (point) 'lumenna-row)))
            (when (and row (equal (lumenna--row-key row) key)) (setq found (point))))
          (forward-line 1))
        found))))

(defun lumenna--insert-listing (listing)
  "Draw LISTING, (HEADING . ROWS), from the top of this buffer.
The heading is a line, or a row of its own: the day's, which carries the
day's actions."
  (let ((start (point))
        (heading (car listing)))
    (insert (if (stringp heading) heading (plist-get heading :text)))
    (add-text-properties start (point) '(face lumenna-heading lumenna-level 1))
    (unless (stringp heading)
      (put-text-property start (point) 'lumenna-row heading))
    (insert "\n"))
  (mapc #'lumenna--insert-row (cdr listing))
  (when (and (null (cdr listing)) lumenna--empty (not (string-empty-p lumenna--empty)))
    ;; No row: only what the list says, at the level under the heading.
    (let ((start (point)))
      (insert lumenna--empty)
      (add-text-properties start (point) '(face lumenna-quiet lumenna-level 2))
      (insert "\n"))))

(defun lumenna--held-view ()
  "This buffer's list's row in hand and landing count, as the view has them."
  (if lumenna--held (plist-get lumenna--view lumenna--held) lumenna--view))

(defun lumenna--redraw ()
  "Draw this buffer from its view.
Point stays on the row it was on, else on the line that took its place, and
goes to the row in hand only when the app moved it there (its landing went
up): never because a sync or a minute redrew the list."
  (when (and lumenna--draw lumenna--view)
    (let* ((inhibit-read-only t)
           (window (get-buffer-window (current-buffer) t))
           (at (if window (window-point window) (point)))
           (key (lumenna--row-key (get-text-property (save-excursion (goto-char at) (line-beginning-position)) 'lumenna-row)))
           (line (line-number-at-pos at))
           (held (lumenna--held-view))
           (landing (plist-get held :landing))
           (land (and (numberp landing) (or (null lumenna--landed) (> landing lumenna--landed))))
           (listing (funcall lumenna--draw)))
      (setq lumenna--landed landing)
      (erase-buffer)
      (lumenna--insert-listing listing)
      (goto-char (point-min))
      (if-let* ((found (lumenna--find (if land (plist-get held :focus) key))))
          (goto-char found)
        (forward-line (1- (min line (count-lines (point-min) (point-max)))))
        (when (and (bobp) (cdr listing)) (forward-line 1)))
      (when window (set-window-point window (point))))))

(defun lumenna-refresh ()
  "Read this buffer's list again, keeping point on the same item."
  (interactive)
  (cond (lumenna--screen
         (jsonrpc-notify (lumenna--connection) 'app.tick (make-hash-table))
         (lumenna--settle)
         (lumenna--redraw))
        (lumenna--source
         (let* ((inhibit-read-only t)
                (key (lumenna--row-key (lumenna--row-here)))
                (line (line-number-at-pos))
                (listing (funcall lumenna--source)))
           (erase-buffer)
           (lumenna--insert-listing listing)
           (goto-char (point-min))
           (if-let* ((found (lumenna--find key)))
               (goto-char found)
             (forward-line (1- (min line (count-lines (point-min) (point-max)))))
             (when (and (bobp) (cdr listing)) (forward-line 1)))))))

(defun lumenna-refresh-all ()
  "Redraw every Lumenna buffer that is not one of the app's screens."
  (dolist (buffer (buffer-list))
    (with-current-buffer buffer
      (when (and lumenna--source (derived-mode-p 'lumenna-list-mode))
        (condition-case err
            (lumenna-refresh)
          (user-error (message "%s" (cadr err))))))))

(defvar-local lumenna--activate nil
  "What RET does: a function of the row at point.")

(defun lumenna-activate ()
  "Do the main thing for the item at point."
  (interactive)
  (if lumenna--activate
      (funcall lumenna--activate (lumenna-row))
    (user-error "Nothing to do here")))

;;;; The app's questions, asked in the minibuffer

(defun lumenna--prompt (&rest parts)
  "PARTS that are not empty, as one minibuffer prompt, ending as Emacs's do.
A part ending in a full stop is followed by the next directly; the prompt
ends \": \", or \"? \" when its last part is a question."
  (let ((text (string-join
               (mapcar (lambda (p) (string-remove-suffix "." (string-trim-right p)))
                       (seq-remove (lambda (p) (or (null p) (string-blank-p p))) parts))
               ". ")))
    (if (string-suffix-p "?" text) (concat text " ") (concat (string-remove-suffix ":" text) ": "))))

(defun lumenna--with-default (prompt default)
  "PROMPT, which ends \": \", saying DEFAULT as Emacs's prompts do: \"(default 2)\"."
  ;; `format-prompt' reads its prompt as a format; the core's words are text.
  (format-prompt (string-replace "%" "%%" (string-remove-suffix ": " prompt)) default))

(defun lumenna--question-title (question)
  "QUESTION's title as a prompt starts with it: in sentence case, as prompts are.
A menu's item is Title Case; what the minibuffer asks is a sentence."
  (or (plist-get question :sentence) (plist-get question :title)))

(defun lumenna--unique (lines)
  "LINES, an alist keyed by text, with each repeated key told apart by a number."
  (let ((seen (make-hash-table :test #'equal)))
    (mapcar (lambda (line)
              (let ((count (puthash (car line) (1+ (gethash (car line) seen 0)) seen)))
                (if (= count 1) line (cons (format "%s (%d)" (car line) count) (cdr line)))))
            lines)))

(defvar lumenna--forms nil
  "How each form the app leaves to a client opens.
An alist from (SUBJECT . KIND), as an action names them, to a function of
the action.")

(defun lumenna-define-form (subject kind function)
  "Open FUNCTION's form for an action of SUBJECT and KIND that asks for one.
FUNCTION is called with the action, from the buffer it was asked on."
  (setf (alist-get (cons subject kind) lumenna--forms nil nil #'equal) function))

(defvar lumenna--typed nil
  "What was last typed for a text question, kept for the question asked again.")

(defun lumenna--ask-text (action question refusal)
  "Read the line ACTION's text QUESTION asks for; sent as typed, even empty.
Asked again after REFUSAL, the core's reason, with what was typed."
  (let ((prompt (lumenna--prompt refusal (lumenna--question-title question) (plist-get question :hint)
                                 (plist-get question :label)))
        (initial (if refusal lumenna--typed (plist-get question :initial))))
    (setq lumenna--typed
          (cond ((equal (plist-get action :kind) "change_query")
                 (lumenna-read-line prompt "filter" initial 'lumenna-filter-history))
                ;; Not the action's own question: how long a pick is meant to take.
                ((not (equal question (plist-get action :question)))
                 (read-from-minibuffer prompt initial))
                (t (read-string prompt initial))))))

(defun lumenna--ask-pick (asking)
  "Pick one of what ASKING offers: Put in a Block's ends in Another Day, the app's."
  (let* ((question (plist-get asking :question))
         (choices (append (plist-get (plist-get asking :choices) :choices) nil))
         (lines (append (plist-get lumenna--lines :choices) nil))
         (named (lumenna--unique (cl-mapcar (lambda (choice line) (cons (plist-get line :text) choice))
                                            choices lines)))
         ;; Why a choice is offered ("Suggested because ...") is its description:
         ;; beside it as completion shows it, never in the name typed.
         (completion-extra-properties
          (list :annotation-function
                (lambda (line)
                  (let* ((choice (cdr (assoc line named)))
                         (reason (plist-get choice :reason)))
                    (concat "  " (lumenna--swatch (plist-get choice :swatch))
                            (if (and reason (not (string-empty-p reason))) reason ""))))))
         (picked (cdr (assoc (completing-read (lumenna--prompt (plist-get asking :refusal) (plist-get question :title))
                                              named nil t)
                             named))))
    (when picked (list :answer "picked" :id (plist-get picked :id)))))

(defun lumenna--ask-choose (question)
  "One of QUESTION's answers, by its title."
  (let* ((answers (mapcar (lambda (a) (cons (plist-get a :title) (plist-get a :id)))
                          (append (plist-get question :answers) nil)))
         (title (completing-read (lumenna--prompt (plist-get question :message) (plist-get question :title))
                                 answers nil t)))
    (list :answer "picked" :id (cdr (assoc title answers)))))

(defun lumenna--ask (asking)
  "Ask ASKING, the question the app has open, and give its answer.
Nil to leave it unanswered; `:again' when it was answered another way (a
form opened, another day's blocks asked for)."
  (let* ((question (plist-get asking :question))
         (action (plist-get asking :action))
         (refusal (plist-get asking :refusal)))
    (pcase (plist-get question :ask)
      ("immediate" (list :answer "yes"))
      ("confirm"
       ;; What going ahead does, then the question, which the answer follows.
       (and (yes-or-no-p (lumenna--prompt refusal (plist-get question :message) (plist-get question :title)))
            (list :answer "yes")))
      ("text" (when-let* ((text (lumenna--ask-text action question refusal)))
                (list :answer "text" :text text)))
      ("pick" (lumenna--ask-pick asking))
      ("choose" (lumenna--ask-choose question))
      ("form"
       (let ((open (alist-get (cons (plist-get action :subject) (plist-get action :kind)) lumenna--forms nil nil #'equal)))
         (unless open (user-error "%s has no form in this client yet" (plist-get action :title)))
         ;; The form is this client's: the app's question is let go, and the form opens.
         (lumenna-send "Cancel")
         (lumenna--settle)
         (funcall open action)
         :again))
      (other (user-error "This client cannot ask a question of kind %s; update it" other)))))

(defvar lumenna--block-form-function nil
  "What fills in the app's block form while it is open: `lumenna-day''s.")

(defun lumenna--follow (&optional quietly)
  "Wait until the app is still, then answer what it opened on this screen.
Each question it asks is asked in the minibuffer, one after another -- a
refusal asks again with why -- and the block form is filled in; what it
opened as a screen of its own then opens in a buffer.  QUIETLY leaves an
action's sentence unsaid when nothing came of it."
  (let ((buffer (current-buffer)))
    (lumenna--settle)
    (catch 'done
      (dotimes (_ 50)
        (with-current-buffer buffer
          (let ((asking (plist-get lumenna--view :asking)))
            (cond
             (asking
              (let ((answer (condition-case nil
                                (lumenna--ask asking)
                              (quit (lumenna-send "Cancel") (lumenna--settle)
                                    (setq lumenna--acting nil)
                                    (signal 'quit nil)))))
                (cond ((eq answer :again) (lumenna--settle))
                      (answer (lumenna-send (list :Answer answer)) (lumenna--settle)
                              ;; Answered and nothing moved: asking again would loop.
                              (when (equal asking (plist-get lumenna--view :asking)) (throw 'done nil)))
                      (t (lumenna-send "Cancel") (lumenna--settle)
                         (setq lumenna--acting nil)
                         (unless quietly (message "Nothing done"))
                         (throw 'done nil)))))
             ((and (plist-get lumenna--view :block_form) lumenna--block-form-function)
              (funcall lumenna--block-form-function)
              (lumenna--settle))
             (t (throw 'done nil)))))))
    ;; What a hook opens is left current, as the window it shows in makes it.
    (when (buffer-live-p buffer)
      (set-buffer buffer)
      (run-hooks 'lumenna-followed-hook))))

;;;; Actions: what can be done to a row, as the app offers it

;; Which actions a row has, what each is called and what it asks are the
;; app's: every row carries its `:actions', in short.  A key runs "the action of this
;; kind on the row at point", so it does nothing the row does not offer, and
;; `.' offers them all by name.

(defvar-local lumenna--actions-function nil
  "A function of no arguments giving the actions here, or nil for the row's own.
One task's buffer offers the task's, whatever field point is on.")

(defun lumenna-actions ()
  "The actions offered on what point is on, in the app's order."
  (append (if lumenna--actions-function
              (funcall lumenna--actions-function)
            (plist-get (lumenna--row-here) :actions))
          nil))

(defun lumenna--side ()
  "The side list this buffer lists, as the app names it."
  (pcase lumenna--held
    (:sidebar_held "Sidebar") (:reminders_held "Reminders") (:devices_held "Devices")))

(defun lumenna--hold (row)
  "Put ROW in hand, the app's way: this list's row, or a side list's."
  (when-let* ((key (and row (lumenna--row-key row))))
    (lumenna-send (if lumenna--held (list :Hold (vector (lumenna--side) key)) (list :Focus key)))))

(defvar lumenna--acted-on nil
  "The row the last action was done to: what a buffer it opens is named after.")

(defun lumenna--acting-on (row)
  "Which row an action offered on ROW is for, as the app names it.
A task's buffer offers its task's; a side list's row is named there; a row
standing for something else names it as its `:on' (Settings' own buttons)."
  (cond (lumenna--actions-function "Details")
        ((null row) (user-error "Nothing to do that to here"))
        ((plist-get row :on))
        (lumenna--held (list :Held (vector (lumenna--side) (lumenna--row-key row))))
        (t (list :Row (lumenna--row-key row)))))

(defun lumenna-act (action &optional row on)
  "Do ACTION, offered in short on ROW (or on what ON names), the app's way.
The app puts the row in hand and runs its action whole as it has it now; what
it asks is asked."
  (setq lumenna--acted-on row)
  (setq lumenna--acting (format "%s/%s" (plist-get action :subject) (plist-get action :kind)))
  (lumenna-send (list :ActOn (vector (or on (lumenna--acting-on row)) action)))
  (lumenna--follow))

(defun lumenna--choose-action (prompt actions)
  "One of ACTIONS, chosen by its title, asking PROMPT."
  (let ((titles (mapcar (lambda (a) (cons (plist-get a :title) a)) actions)))
    (cdr (assoc (completing-read prompt titles nil t) titles))))

(defun lumenna-act-kind (kinds)
  "Do the action of one of KINDS that point's row offers.
Where it offers several (one Stop Waiting per task waited for), choose one;
where it offers none, the core's sentence says why."
  (let* ((actions (lumenna-actions))
         (row (lumenna--row-here))
         (matching (seq-filter (lambda (a) (member (plist-get a :kind) kinds)) actions)))
    (cond ((cdr matching) (lumenna-act (lumenna--choose-action "Which one: " matching) row))
          (matching (lumenna-act (car matching) row))
          ((null actions) (user-error "Nothing to do here"))
          (t (lumenna--refuse row kinds)))))

(defun lumenna--refuse (row kinds)
  "Run KINDS on ROW, which offers none of them, and refuse in the app's words."
  (lumenna--hold row)
  (let ((lumenna--heard (list 'heard)))
    (lumenna-send (list :Run (vconcat kinds)))
    (lumenna--settle)
    (user-error "%s" (string-join (or (cdr lumenna--heard) '("Nothing to do here")) " "))))

(defun lumenna-act-at-point ()
  "Choose one of the actions offered on this line, by name, and do it."
  (interactive)
  (let ((actions (lumenna-actions)))
    (unless actions (user-error "Nothing to do here"))
    (lumenna-act (lumenna--choose-action "Do: " actions) (lumenna--row-here))))

(defmacro lumenna-define-action (name kinds doc)
  "Define NAME, a command doing the row's action of one of KINDS.
DOC is its documentation."
  (declare (indent 2) (doc-string 3))
  `(defun ,name ()
     ,doc
     (interactive)
     (lumenna-act-kind ',kinds)))

(lumenna-define-action lumenna-act-done ("mark_done" "mark_not_done")
  "Mark the task at point done, or not done if it is.")
(lumenna-define-action lumenna-act-edit ("edit" "edit_task")
  "Open the form of what is at point: a task's details, a block's fields.")
(lumenna-define-action lumenna-act-delete ("delete" "delete_for_good" "unassign" "unpair")
  "Delete what is at point, as it offers.
To the trash, from the trash, out of a block, or a device unpaired.")
(lumenna-define-action lumenna-act-restore ("restore" "restore_day")
  "Bring back what is at point: a task from the trash, a day of a block.")
(lumenna-define-action lumenna-act-rename ("rename")
  "Rename what is at point.")
(lumenna-define-action lumenna-act-move-up ("move_up")
  "Move what is at point one place up among its siblings.")
(lumenna-define-action lumenna-act-move-down ("move_down")
  "Move what is at point one place down among its siblings.")
(lumenna-define-action lumenna-act-move-to-top ("move_to_top_level")
  "Take what is at point out from under its parent.")
(lumenna-define-action lumenna-act-reminders ("reminders")
  "List the reminders of the task or block at point, to add or remove one.")
(lumenna-define-action lumenna-toggle ("collapse" "expand")
  "Fold or unfold what sits under the item at point.")

(defun lumenna-undo ()
  "Undo the last change this device made to the store, wherever it was made."
  (interactive)
  (lumenna--on-a-screen
    (lumenna-send "Undo")
    (lumenna--follow)))

(defun lumenna-redo ()
  "Redo the change most recently undone."
  (interactive)
  (lumenna--on-a-screen
    (lumenna-send "Redo")
    (lumenna--follow)))

;;;; Keys, menus and the list mode

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

(defconst lumenna--minor-words
  '("a" "an" "the" "and" "or" "nor" "but" "as" "at" "by" "for" "from" "in" "into" "of" "on" "to" "under" "with")
  "Words a title leaves in lower case, unless they start it.")

(defun lumenna--title-case (text)
  "TEXT in Title Case, as Emacs's menus have their items.
A word with a capital already, a key or a name, is left as it is."
  (let ((first t) (case-fold-search nil))
    (mapconcat (lambda (word)
                 (prog1 (if (and (not first) (member word lumenna--minor-words))
                            word
                          (if (or (string-empty-p word) (string-match-p "[[:upper:]]" word))
                              word
                            (concat (upcase (substring word 0 1)) (substring word 1))))
                   (setq first nil)))
               (split-string text " ")
               " ")))

(defun lumenna--menu (owner)
  "A menu of OWNER's keys, grouped as its help groups them.
Where two keys run one command, the menu lists it once; the menu shows
whichever key it is reached by."
  (cons "Lumenna"
        (mapcar (lambda (group)
                  (let (seen)
                    (cons (lumenna--title-case (car group))
                          (delq nil (mapcar (lambda (binding)
                                              (unless (memq (nth 2 binding) seen)
                                                (push (nth 2 binding) seen)
                                                (vector (lumenna--title-case (nth 1 binding)) (nth 2 binding))))
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
Subtasks sit under their task, a level deeper; TAB folds them, as the app does.

\\{lumenna-list-mode-map}"
  (setq-local truncate-lines nil)
  (setq-local revert-buffer-function (lambda (&rest _) (lumenna-refresh)))
  ;; Every line is an outline heading, its level the item's depth, for
  ;; Emacsvox's and Emacspeak's outline support; the folding is the app's.
  (setq-local outline-regexp "[^\n]")
  (setq-local outline-level #'lumenna--outline-level)
  (setq-local imenu-create-index-function #'lumenna--imenu-index))

(lumenna-define-keys lumenna-list-mode
  ("Every list"
   ("n" "Next line" next-line)
   ("p" "Previous line" previous-line)
   ("RET" "Do the main thing for this line" lumenna-activate)
   ("." "This line's actions, by name" lumenna-act-at-point)
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
  "Show buffer NAME in MODE, listing SOURCE, a list that is not the app's.
SETTINGS are buffer-local pairs."
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
                    (jsonrpc-request (lumenna--connection) 'app.complete
                                     (list :text text :syntax syntax :cursor (lumenna--byte-offset text cursor)))))
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

(defvar lumenna--reading-for nil
  "The buffer whose screen the minibuffer's line is read for.")

(defun lumenna-read-back ()
  "Say how the line typed so far is understood, before it is entered.
What the other apps show under the field as it is typed: a task's date,
priority and labels, or a filter's meaning and how many tasks it matches."
  (interactive nil minibuffer-mode)
  (let ((text (minibuffer-contents-no-properties))
        (syntax lumenna--syntax)
        (cursor (- (point) (minibuffer-prompt-end))))
    (funcall lumenna-announce-function
             (if (string-blank-p text)
                 "Nothing typed yet"
               (with-current-buffer (or lumenna--reading-for (lumenna--home-buffer))
                 (if (equal syntax "quick-add")
                     (progn (lumenna-send (list :Typing (list :Text (list :text text :cursor (lumenna--byte-offset text cursor)))))
                            (lumenna--settle)
                            (plist-get (plist-get lumenna--view :line) :readback))
                   (lumenna-send (list :Filter text))
                   (lumenna--settle)
                   (plist-get lumenna--lines :readback))))
             nil)))

(defvar lumenna-add-history nil "Quick-add lines typed before.")
(defvar lumenna-filter-history nil "Filters typed before.")

(defun lumenna-read-line (prompt syntax &optional initial history example)
  "Read a line in SYNTAX, \"quick-add\" or \"filter\", asking PROMPT.
The core completes it and reads it back.  INITIAL starts the line; HISTORY
is the history variable; EXAMPLE is what \\<minibuffer-local-map>\\[next-history-element] brings in."
  (let ((lumenna--reading-for (and lumenna--screen (current-buffer))))
    (minibuffer-with-setup-hook
        (lambda ()
          (setq lumenna--syntax syntax)
          (add-hook 'completion-at-point-functions (lumenna--completion syntax) nil t))
      (read-from-minibuffer prompt initial lumenna-minibuffer-map nil history
                            (and example (not (string-empty-p example)) example)))))

;;;###autoload
(defun lumenna-add ()
  "Add a task, written the way you would say it.
Such as \"call the bank tomorrow at 3pm p1 #Home @calls 15m\".  TAB completes
a project or label, and \\<lumenna-minibuffer-map>\\[lumenna-read-back] says how the line is understood
so far.  The line is checked first: an unknown project is said and nothing
is added; an unknown label becomes a new label.  In a project's list the
line starts with the project, as the app has it."
  (interactive)
  (lumenna--on-a-screen
    (let* ((start (plist-get lumenna--view :quick_add))
           (_ (lumenna-send (list :Typing "Open")))
           (text (unwind-protect
                     (lumenna-read-line "Add a task: " "quick-add"
                                        (and start (not (string-empty-p start)) start) 'lumenna-add-history)
                   (lumenna-send (list :Typing "Close")))))
      (when (string-blank-p text) (user-error "Nothing to add"))
      (lumenna-send (list :QuickAdd text))
      (lumenna--follow)
      (when-let* ((refused (plist-get lumenna--view :add_refused)))
        (user-error "%s" refused)))))

;;;; The places, and the menu of everything

(define-derived-mode lumenna-home-mode lumenna-list-mode "Lumenna"
  "Lumenna's places, as every app's sidebar has them: RET opens one.
Under each heading are its projects, labels or saved filters, each with
its own actions; a heading's adds another.

\\{lumenna-home-mode-map}"
  (setq-local lumenna--held :sidebar_held)
  (setq-local lumenna--draw #'lumenna--place-rows)
  (setq-local lumenna--activate #'lumenna--open-place))

(defun lumenna--place-rows ()
  "The app's places, its sidebar, then Settings, as rows.
What sits under a folded row, deeper than it until the next at its depth, is
left out."
  (let ((folded (append (plist-get lumenna--view :sidebar_folded) nil))
        (under nil)
        rows)
    (dolist (entry (append (plist-get lumenna--view :sidebar) nil))
      (let ((depth (or (plist-get entry :depth) 0)))
        (when (and under (<= depth under)) (setq under nil))
        (unless under
          (push (list :key (plist-get entry :key) :text (plist-get entry :text)
                      :title (plist-get entry :title)
                      :depth depth :place (plist-get entry :kind)
                      :actions (plist-get entry :offered) :swatch (plist-get entry :swatch)
                      :role (if (plist-get (plist-get entry :kind) :Group) "heading" "place")
                      :state (and (lumenna--true (plist-get entry :archived)) ["archived"]))
                rows)
          (when (member (plist-get entry :key) folded) (setq under depth)))))
    (cons "Lumenna"
          (nreverse (cons (list :key "Settings" :title "Settings" :place '(:Place "Settings")) rows)))))

(declare-function lumenna-open-place "lumenna-tasks" (place))

(defun lumenna--open-place (row)
  "Open the place ROW names; on a heading, fold what is under it."
  (let ((kind (plist-get row :place)))
    (cond ((plist-get kind :Group) (lumenna-toggle))
          ((equal (plist-get kind :Place) "Settings") (call-interactively #'lumenna-settings))
          (t (lumenna-open-place (plist-get kind :Place))))))

(defun lumenna-heading-action (group)
  "The action the places' GROUP heading offers: \"Projects\", \"Labels\", \"Filters\"."
  (with-current-buffer (lumenna--home-buffer)
    (let ((entry (seq-find (lambda (e) (equal (plist-get (plist-get e :kind) :Group) group))
                           (append (plist-get lumenna--view :sidebar) nil))))
      (or (car (append (plist-get entry :offered) nil))
          (user-error "Nothing adds one here")))))

;;;###autoload
(defun lumenna ()
  "Open Lumenna: its places, one per line.  RET opens one; ? shows every command."
  (interactive)
  (pop-to-buffer-same-window (lumenna--home-buffer)))

(lumenna-define-keys lumenna-home-mode
  ("The place at point"
   ("RET" "Open it, or fold a heading" lumenna-activate)
   ("r" "Rename" lumenna-act-rename)
   ("M-p" "Move Up" lumenna-act-move-up)
   ("M-n" "Move Down" lumenna-act-move-down)
   ("d" "Delete" lumenna-act-delete)))

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
   ("h" "History" lumenna-history)
   ("x" "Trash" lumenna-trash)
   ("L" "All of Lumenna's places" lumenna))
  ("Do"
   ("a" "Add a task" lumenna-add)
   ("/" "Search or filter" lumenna-search)
   ("u" "Undo" lumenna-undo)
   ("y" "Redo" lumenna-redo)
   ("n" "Sync now" lumenna-sync-now)
   ("r" "Reminders due" lumenna-reminders-due))
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
(require 'lumenna-proposal)
(require 'lumenna-organise)
(require 'lumenna-settings)
(require 'lumenna-reminders)
(require 'lumenna-voice)
(with-eval-after-load 'emacsvox-aural-submission
  (require 'lumenna-emacsvox))

;;; lumenna.el ends here
