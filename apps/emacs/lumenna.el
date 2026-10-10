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
  "How a result is said: called with the announcement and a list of notices."
  :type 'function)

(defvar lumenna-row-inserted-functions nil
  "Called with START, END and the ROW plist after each row is drawn.
`lumenna-emacsvox' uses this to attach semantic facts to the line.")

(defvar lumenna-changed-functions nil
  "Called with the EVENT and its RESULT after a change succeeds.
EVENT is the RPC method, or for an action SUBJECT/KIND, as \"task/mark_done\".
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

(defvar lumenna--words (make-hash-table :test #'equal)
  "The core's fixed words, by the form function and parameters that gave them.")

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
  (clrhash lumenna--words)
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
  (lumenna--wrote method (apply #'lumenna-call method params)))

(defun lumenna--wrote (event result)
  "Redraw after a change, run the change hooks with EVENT, and say RESULT.
EVENT is the RPC method, or for an action `SUBJECT/KIND'.  Returns RESULT."
  (lumenna-refresh-all)
  (run-hook-with-args 'lumenna-changed-functions event result)
  (lumenna-say result)
  result)

(defun lumenna-project-reference (name)
  "Project NAME as a filter or quick-add line names it: `#Work', `#\"Home Office\"'."
  (lumenna--value "form.project_reference" :name name))

(defun lumenna-label-reference (name)
  "Label NAME as a filter or quick-add line names it: `@calls', `@\"deep work\"'."
  (lumenna--value "form.label_reference" :name name))

(defun lumenna--value (method &rest params)
  "The value the form function METHOD computes from PARAMS."
  (plist-get (apply #'lumenna-call method params) :value))

(defun lumenna-words (method &rest params)
  "What the form function METHOD says for PARAMS, asked once.
The forms' fields (`form.task_form', `form.block_form'), pairing's sentences
\(`form.pairing_words'), a fixed text in sentence case: the core's words,
which never change while it runs."
  (let ((key (cons method params)))
    (or (gethash key lumenna--words)
        (puthash key (apply #'lumenna--value method params) lumenna--words))))

(defun lumenna-sentence (text)
  "Fixed TEXT, a button or a dialog's title, in sentence case, as a prompt is.
The core's: \"Do These Words Match?\" asked in the minibuffer."
  (lumenna-words "form.sentence_case" :text text))

(defun lumenna-form (form)
  "FORM's fields, \"task\" or \"block\", as the core words them: label, hint, options."
  (append (lumenna-words (format "form.%s_form" form)) nil))

(defun lumenna-form-field (form key)
  "The field KEY, a symbol, of FORM, \"task\" or \"block\"."
  (seq-find (lambda (field) (equal (plist-get field :key) (symbol-name key))) (lumenna-form form)))

(defun lumenna-field-prompt (field &rest more)
  "A prompt for FIELD: its hint, then MORE, then its label.
\\<minibuffer-local-map>\\[next-history-element] offers its example, through `lumenna-read-field'.
The answer follows the label, as a form's field follows its name."
  (apply #'lumenna--prompt (plist-get field :hint) (append more (list (plist-get field :label)))))

(defun lumenna--with-default (prompt default)
  "PROMPT, which ends \": \", saying DEFAULT as Emacs's prompts do: \"(default 2)\"."
  ;; `format-prompt' reads its prompt as a format; the core's words are text.
  (format-prompt (string-replace "%" "%%" (string-remove-suffix ": " prompt)) default))

(defun lumenna-read-field (field prompt &optional initial)
  "Read FIELD's text, asking PROMPT, starting from INITIAL.
The field's example, as another app shows it in an empty field, is the
minibuffer's future history: \\<minibuffer-local-map>\\[next-history-element] brings it in.  Empty is
an answer, never the example."
  (let ((example (plist-get field :example)))
    (read-from-minibuffer prompt initial nil nil nil (and example (not (string-empty-p example)) example))))

(defun lumenna-field-options (field)
  "FIELD's options, as (TITLE . ID) for `completing-read'."
  (mapcar (lambda (o) (cons (plist-get o :title) (plist-get o :id))) (append (plist-get field :options) nil)))

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

(defun lumenna-time (time)
  "TIME, `HH:MM' as the core sends it, in Emacs's own clock.
That is `display-time-24hr-format', as the mode line's clock follows:
\"15:00\" when it is set, else \"3:00 PM\"."
  (if (or display-time-24hr-format
          (not (and (stringp time) (string-match "\\`\\([0-9]+\\):\\([0-9]+\\)\\'" time))))
      time
    (let ((hour (string-to-number (match-string 1 time))))
      (format "%d:%s %s" (1+ (% (+ hour 11) 12)) (match-string 2 time) (if (< hour 12) "AM" "PM")))))

(defun lumenna--due (row)
  "When ROW is due, its time in Emacs's clock: \"due tomorrow at 3:00 PM\"."
  (when-let* ((due (plist-get row :due)))
    (if-let* ((time (plist-get row :due_time)))
        (format "%s at %s" due (lumenna-time time))
      due)))

(defun lumenna-describe (row)
  "ROW as one line of words: the title, done, when due, the value, then the states.
\"ready\" is true of almost every task, so saying it everywhere would bury
the states that mean something."
  (string-join
   (delq nil (append (list (plist-get row :title)
                           (and (lumenna--true (plist-get row :checked)) "done")
                           (lumenna--due row)
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

(defvar-local lumenna--empty nil
  "What this list says when it has nothing in it: the listing's own `empty'.
Its source sets it, from what the core returned.")

(defun lumenna-listing (title listing &optional rows after)
  "A heading and ROWS, as a buffer's source returns them.
ROWS are LISTING's `:rows' unless given.  The heading is TITLE, then
LISTING's announcement, then the strings AFTER.  An empty list leaves its
count out of the heading, since the line under it says it is empty, in the
core's words, kept for the buffer."
  (let ((rows (append (or rows (plist-get listing :rows)) nil)))
    (setq-local lumenna--empty (plist-get listing :empty))
    (cons (string-join (delq nil (append (list title (and (or rows (null lumenna--empty))
                                                         (plist-get listing :announcement)))
                                         after))
                       ", ")
          rows)))

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
      (when (and (null (cdr listing)) lumenna--empty (not (string-empty-p lumenna--empty)))
        ;; No row: only what the list says, at the level under the heading.
        (let ((start (point)))
          (insert lumenna--empty)
          (add-text-properties start (point) '(face lumenna-quiet lumenna-level 2))
          (insert "\n")))
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

;;;; Actions: what can be done to a row, as the core offers it

;; Which actions a row has, what each is called and what it asks are the
;; core's: every listed record carries its `:actions'.  This asks each
;; question in the minibuffer and hands the answer back through `act'.  A key
;; runs "the action of this kind on the row at point", so it does nothing the
;; row does not offer, and `.' offers them all by name.

(defvar-local lumenna--actions-function nil
  "A function of no arguments giving the actions here, or nil for the row's own.
One task's buffer offers the task's, whatever field point is on.")

(defvar lumenna--forms nil
  "How each form the core leaves to a client opens.
An alist from (SUBJECT . KIND), as an action names them, to a function of
the action and the row it came from.")

(defun lumenna-define-form (subject kind function)
  "Open FUNCTION's form for an action of SUBJECT and KIND that asks for one.
FUNCTION is called with the action and the row it was offered on."
  (setf (alist-get (cons subject kind) lumenna--forms nil nil #'equal) function))

(defun lumenna-actions ()
  "The actions offered on what point is on, in the core's order."
  (append (if lumenna--actions-function
              (funcall lumenna--actions-function)
            (plist-get (lumenna-row) :actions))
          nil))

(defun lumenna--row-here ()
  "The row on this line, or nil."
  (get-text-property (line-beginning-position) 'lumenna-row))

(defun lumenna--spoken-day (iso)
  "ISO as a person says it: `Sunday 4 October 2026'."
  (let ((time (encode-time (append '(0 0 12) (reverse (mapcar #'string-to-number (split-string iso "-")))))))
    (format-time-string "%A %-d %B %Y" time)))

(defun lumenna--unique (lines)
  "LINES, an alist keyed by text, with each repeated key told apart by a number."
  (let ((seen (make-hash-table :test #'equal)))
    (mapcar (lambda (line)
              (let ((count (puthash (car line) (1+ (gethash (car line) seen 0)) seen)))
                (if (= count 1) line (cons (format "%s (%d)" (car line) count) (cdr line)))))
            lines)))

(defun lumenna--choice-line (choice)
  "CHOICE as a line to choose by.
A block's day and times, else its title and detail."
  (if-let* ((date (plist-get choice :date)))
      (format "%s, %s to %s, %s" (lumenna--spoken-day date)
              (lumenna-time (plist-get choice :start)) (lumenna-time (plist-get choice :end))
              (plist-get choice :title))
    (string-join (delq nil (list (plist-get choice :title) (plist-get choice :detail))) ", ")))

(defun lumenna--prompt (&rest parts)
  "PARTS that are not empty, as one minibuffer prompt, ending as Emacs's do.
A part ending in a full stop is followed by the next directly; the prompt
ends \": \", or \"? \" when its last part is a question."
  (let ((text (string-join
               (mapcar (lambda (p) (string-remove-suffix "." (string-trim-right p)))
                       (seq-remove (lambda (p) (or (null p) (string-blank-p p))) parts))
               ". ")))
    (if (string-suffix-p "?" text) (concat text " ") (concat (string-remove-suffix ":" text) ": "))))

(defun lumenna--question-title (question)
  "QUESTION's title as a prompt starts with it: in sentence case, as prompts are.
A menu's item is Title Case; what the minibuffer asks is a sentence."
  (or (plist-get question :sentence) (plist-get question :title)))

(defun lumenna-ask-line (method &optional history)
  "The line one of the client's own questions asks for, in the core's words.
METHOD gives the question: `form.go_to_day', `form.length'.  HISTORY is the
history variable."
  (let ((question (lumenna-words method)))
    (read-from-minibuffer (lumenna--prompt (lumenna--question-title question) (plist-get question :hint)
                                           (plist-get question :label))
                          (plist-get question :initial) nil nil history)))

(defun lumenna--ask-text (action question)
  "Read the line ACTION's text QUESTION asks for; sent as typed, even empty."
  (let ((prompt (lumenna--prompt (lumenna--question-title question) (plist-get question :hint)
                                 (plist-get question :label)))
        (initial (plist-get question :initial)))
    (if (equal (plist-get action :kind) "change_query")
        (lumenna-read-line prompt "filter" initial 'lumenna-filter-history)
      (read-string prompt initial))))

(defun lumenna--ask-pick (action question)
  "Pick one of what the core offers for ACTION, asking QUESTION; nil if nothing is.
When nothing is offered, the core's sentence says why."
  (let* ((offered (lumenna-call "choices" :action action))
         (choices (append (plist-get offered :choices) nil)))
    (if (null choices)
        (progn (lumenna-say offered) nil)
      (let* ((lines (lumenna--unique (mapcar (lambda (c) (cons (lumenna--choice-line c) c)) choices)))
             (picked (cdr (assoc (completing-read (lumenna--prompt (plist-get question :title)) lines nil t)
                                 lines)))
             (length (when (plist-get question :length)
                       (lumenna-ask-line "form.length"))))
        (list :answer "picked" :id (plist-get picked :id) :length length)))))

(defun lumenna--ask-choose (question)
  "One of QUESTION's answers, by its title."
  (let* ((answers (mapcar (lambda (a) (cons (plist-get a :title) (plist-get a :id)))
                          (append (plist-get question :answers) nil)))
         (title (completing-read (lumenna--prompt (plist-get question :message) (plist-get question :title))
                                 answers nil t)))
    (list :answer "picked" :id (cdr (assoc title answers)))))

(defun lumenna-act (action &optional row)
  "Ask ACTION's question, then do it; ROW is the row it was offered on.
A form is this client's own and opens instead.  Returns the change, or nil
when nothing was sent."
  (let* ((question (plist-get action :question))
         (answer
          (pcase (plist-get question :ask)
            ("immediate" (list :answer "yes"))
            ("form"
             (let ((open (alist-get (cons (plist-get action :subject) (plist-get action :kind))
                                    lumenna--forms nil nil #'equal)))
               (unless open (user-error "%s has no form in this client yet" (plist-get action :title)))
               (funcall open action row)
               nil))
            ("confirm"
             ;; What going ahead does, then the question, which the answer follows.
             (if (yes-or-no-p (lumenna--prompt (plist-get question :message) (plist-get question :title)))
                 (list :answer "yes")
               (message "Nothing done")
               nil))
            ("text" (list :answer "text" :text (lumenna--ask-text action question)))
            ("pick" (lumenna--ask-pick action question))
            ("choose" (lumenna--ask-choose question))
            (other (user-error "This client cannot ask a question of kind %s; update it" other)))))
    (when answer
      (lumenna--wrote (format "%s/%s" (plist-get action :subject) (plist-get action :kind))
                      (lumenna-call "act" :action action :answer answer)))))

(defun lumenna--titles (actions)
  "ACTIONS' titles, as one phrase."
  (string-join (mapcar (lambda (a) (plist-get a :title)) actions) ", "))

(defun lumenna--choose-action (prompt actions)
  "One of ACTIONS, chosen by its title, asking PROMPT."
  (let ((titles (mapcar (lambda (a) (cons (plist-get a :title) a)) actions)))
    (cdr (assoc (completing-read prompt titles nil t) titles))))

(defun lumenna--not-offered (kind actions row)
  "Why ROW, offering ACTIONS, does not offer KIND.
The core's sentence, then what the row does offer."
  (if (null actions)
      "Nothing to do here"
    (let ((why (plist-get (lumenna-call "form.not_offered" :kind kind
                                        :subject (plist-get (car actions) :subject)
                                        :this_device (and (lumenna--true (plist-get (plist-get row :device) :this_device)) t))
                          :value)))
      (format "%s This offers %s" why (lumenna--titles actions)))))

(defun lumenna-act-kind (kinds)
  "Do the action of one of KINDS that point's row offers.
Where it offers several (one Stop Waiting per task waited for), choose one."
  (let* ((actions (lumenna-actions))
         (row (lumenna--row-here))
         (matching (seq-filter (lambda (a) (member (plist-get a :kind) kinds)) actions)))
    (cond ((null matching)
           (user-error "%s" (lumenna--not-offered (car kinds) actions row)))
          ((null (cdr matching)) (lumenna-act (car matching) row))
          (t (lumenna-act (lumenna--choose-action "Which one: " matching) row)))))

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

(defun lumenna-read-line (prompt syntax &optional initial history example)
  "Read a line in SYNTAX, \"quick-add\" or \"filter\", asking PROMPT.
The core completes it and reads it back.  INITIAL starts the line; HISTORY
is the history variable; EXAMPLE is what \\<minibuffer-local-map>\\[next-history-element] brings in."
  (minibuffer-with-setup-hook
      (lambda ()
        (setq lumenna--syntax syntax)
        (add-hook 'completion-at-point-functions (lumenna--completion syntax) nil t))
    (read-from-minibuffer prompt initial lumenna-minibuffer-map nil history
                          (and example (not (string-empty-p example)) example))))

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

(define-derived-mode lumenna-home-mode lumenna-list-mode "Lumenna"
  "Lumenna's places, as every app's sidebar has them: RET opens one.
Under each heading are its projects, labels or saved filters, each with
its own actions; a heading's adds another.

\\{lumenna-home-mode-map}"
  (setq-local lumenna--activate #'lumenna--open-place))

(declare-function lumenna-tasks "lumenna-tasks" (&optional query title prefix))
(declare-function lumenna-open-project "lumenna-organise" (name))
(declare-function lumenna-open-label "lumenna-organise" (name))

(defconst lumenna--place-commands
  '(("Today" . lumenna-today) ("Tasks" . lumenna-tasks) ("Blocks" . lumenna-blocks)
    ("Trash" . lumenna-trash) ("Settings" . lumenna-settings))
  "The command opening each of the core's places by name.
And Settings, which is this client's.")

(defun lumenna--place-rows ()
  "The core's places (`places'), then Settings, as rows."
  (let ((places (lumenna-call "places")))
    (cons "Lumenna"
          (append
           (mapcar (lambda (entry)
                     (list :key (format "%S" (plist-get entry :kind)) :title (plist-get entry :text)
                           :depth (plist-get entry :depth) :place (plist-get entry :kind)
                           :actions (plist-get entry :actions)
                           :role (if (plist-get (plist-get entry :kind) :Group) "heading" "place")
                           :state (and (lumenna--true (plist-get entry :archived)) ["archived"])))
                   (append (plist-get places :entries) nil))
           (list (list :key "Settings" :title "Settings" :place '(:Place "Settings")))))))

(defun lumenna--open-place (row)
  "Open the place ROW names; on a heading, fold what is under it."
  (let* ((kind (plist-get row :place))
         (place (plist-get kind :Place)))
    (cond ((plist-get kind :Group) (lumenna-toggle))
          ((stringp place)
           (call-interactively (or (cdr (assoc place lumenna--place-commands))
                                   (user-error "This client cannot open %s yet" place))))
          ((plist-get place :Project) (lumenna-open-project (plist-get place :Project)))
          ((plist-get place :Label) (lumenna-open-label (plist-get place :Label)))
          ((plist-get place :Filter)
           (let ((filter (plist-get place :Filter)))
             (lumenna-tasks (plist-get filter :query) (plist-get filter :name)))))))

(defun lumenna-heading-action (group)
  "The action the places' GROUP heading offers: \"Projects\", \"Labels\", \"Filters\"."
  (let ((entry (seq-find (lambda (e) (equal (plist-get (plist-get e :kind) :Group) group))
                         (append (plist-get (lumenna-call "places") :entries) nil))))
    (or (car (append (plist-get entry :actions) nil))
        (user-error "Nothing adds one here"))))

;;;###autoload
(defun lumenna ()
  "Open Lumenna: its places, one per line.  RET opens one; ? shows every command."
  (interactive)
  (lumenna--show-list "*Lumenna*" #'lumenna-home-mode #'lumenna--place-rows))

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
