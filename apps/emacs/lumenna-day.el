;;; lumenna-day.el --- Lumenna's day planner and blocks -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Elijah Massey
;; SPDX-License-Identifier: MPL-2.0

;;; Commentary:

;; The day as it is lived, a screen of the app's: blocks in time order with
;; their sittings under them, free time and now as lines of their own, and the
;; blocks cancelled for the day so they can be put back.  Today opens on now
;; the first time.  Also the list of every block series, and the app's block
;; form, asked in the minibuffer.

;;; Code:

(require 'lumenna)

(declare-function lumenna-open-place "lumenna-tasks" (place))
(declare-function lumenna--list-rows "lumenna-tasks" ())

(defun lumenna--day-rows ()
  "The day this buffer's screen shows: its heading, which carries the day's own
actions, then each row in the app's words and order."
  (let* ((day (plist-get lumenna--view :day))
         (items (append (plist-get day :items) nil))
         (lines (append (plist-get lumenna--lines :day) nil))
         (rows (delq nil
                     (cl-mapcar
                      (lambda (item line)
                        (unless (lumenna--true (plist-get item :hidden))
                          (let ((kind (plist-get item :kind))
                                (sitting (plist-get item :sitting)))
                            (list :key (plist-get item :key) :text (plist-get line :text) :title (plist-get line :title)
                                  :depth (plist-get item :depth) :actions (plist-get item :offered)
                                  :role (pcase kind ("sitting" "assignment") (_ kind))
                                  :when (plist-get (plist-get item :block) :when)
                                  :block (plist-get item :block) :sitting sitting
                                  :state (or (plist-get (plist-get item :block) :details)
                                             (plist-get sitting :details))
                                  :face (pcase kind
                                          ("free" 'lumenna-quiet) ("cancelled" 'lumenna-quiet) ("now" 'lumenna-now)
                                          ("sitting" (and (lumenna--true (plist-get sitting :running)) 'lumenna-running)))))))
                      items lines))))
    (setq-local lumenna--empty nil)
    (if (null day)
        (list (lumenna-words :nothing_here))
      (cons (append (car rows) (list :heading t)) (cdr rows)))))

(define-derived-mode lumenna-day-mode lumenna-list-mode "Lumenna Day"
  "A day: blocks with their sittings under them, free time, and now.
RET edits a block, shows a sitting's task, adds a block in free time, or puts
a cancelled day back.  [ and ] move between days.  On the first line, . offers
the day's own actions: Plan the Day, Replan from Now and the rest.

\\{lumenna-day-mode-map}"
  (lumenna--mode-name '(group "day"))
  (setq-local lumenna--draw #'lumenna--day-rows)
  (setq-local lumenna--activate #'lumenna--day-activate))

;;;###autoload
(defun lumenna-today ()
  "Show today, opening on now."
  (interactive)
  (lumenna--show-screen (lumenna--buffer-name (lumenna-word '(group "day"))) #'lumenna-day-mode (list (list :Open "Today"))))

(defun lumenna--day-move (step)
  "Move through the days by STEP, as the app names it: Previous, Next, Now, Ask."
  (lumenna-send (list :Day step))
  (lumenna--follow))

(defun lumenna-day-previous ()
  "Show the day before."
  (interactive)
  (lumenna--day-move "Previous"))

(defun lumenna-day-next ()
  "Show the day after."
  (interactive)
  (lumenna--day-move "Next"))

(defun lumenna-day-today ()
  "Show today, with now in hand."
  (interactive)
  (lumenna--day-move "Now"))

(defun lumenna-day-go-to ()
  "Show a day: any date phrase, such as \"next friday\" or \"2026-12-01\"."
  (interactive)
  (lumenna--day-move "Ask"))

;; Every action on a block, a sitting, free time or a cancelled day is the
;; row's own; these are the keys for them.
(lumenna-define-action lumenna-act-assign ("assign_task")
  "Put a task in the block at point, for a sitting.")
(lumenna-define-action lumenna-act-cancel-day ("cancel_day")
  "Cancel the repeating block at point for this day alone.")
(lumenna-define-action lumenna-act-timer ("start_timer" "pause_timer" "resume_timer")
  "Start the sitting's timer, pause it while it runs, or resume it.")
(lumenna-define-action lumenna-act-stop-timer ("stop_timer")
  "Stop the sitting's timer, ending the sitting, running or paused.")
(lumenna-define-action lumenna-act-planned-length ("planned_length")
  "Set how long the sitting at point is meant to take, or clear it.")
(lumenna-define-action lumenna-act-log-minutes ("log_minutes")
  "Record the whole of the sitting at point, replacing what was logged.")

(lumenna-define-action lumenna-act-end-now ("end_now")
  "End the block under way now.")
(lumenna-define-action lumenna-act-extend ("extend")
  "Give the block under way 15 minutes more.")
(lumenna-define-action lumenna-act-carry ("carry_forward" "carry_rest")
  "Carry what a sitting, or a block, left short forward to a block that takes it.")

(defun lumenna--day-heading ()
  "The day's heading, as its row: the day's own actions are its."
  (save-excursion (goto-char (point-min)) (get-text-property (point) 'lumenna-row)))

(defun lumenna--day-act (kind)
  "Do the day's own action of KIND, wherever point is.
A day that does not offer it says the app's sentence."
  (let* ((heading (lumenna--day-heading))
         (action (seq-find (lambda (a) (equal (plist-get a :kind) kind)) (append (plist-get heading :actions) nil))))
    ;; The day's own: run whatever row is in hand, which stays where it is.
    (if action
        (progn (setq lumenna--acting (format "day/%s" kind))
               (lumenna-send (list :RunDay (vector kind)))
               (lumenna--follow))
      (let ((lumenna--heard (list 'heard)))
        (lumenna-send (list :RunDay (vector kind)))
        (lumenna--settle)
        (user-error "%s" (string-join (or (cdr lumenna--heard) (list (lumenna-word :nothing_to_do))) " "))))))

(defun lumenna-day-plan ()
  "Plan the day shown: each work block still to come given its most urgent tasks."
  (interactive)
  (lumenna--day-act "plan_day"))

(defun lumenna-day-replan ()
  "Replan the rest of today from now."
  (interactive)
  (lumenna--day-act "replan"))

(defun lumenna-day-what-now ()
  "Choose what to work on now, from what the core suggests, and start on it."
  (interactive)
  (lumenna--day-act "what_now"))

(defun lumenna-day-end ()
  "End the day: the block under way ends now, the blocks after it are cleared."
  (interactive)
  (lumenna--day-act "end_day"))

(defun lumenna-day-clear ()
  "Clear the rest of the day: the blocks still to come today."
  (interactive)
  (lumenna--day-act "clear_day"))

(defun lumenna--day-activate (row)
  "RET on ROW, the core's Enter (`open'): edit a block, open a sitting's task,
add a block in free time, put a cancelled day back.  The day's heading and
now are lines to hear again."
  (if (member (plist-get row :role) '("summary" "now"))
      (message "%s" (plist-get row :text))
    (lumenna-press "open")))

(defun lumenna-day-show (date)
  "Show DATE, any date phrase, in this day buffer: as Go to Day would."
  (lumenna-send (list :Day "Ask"))
  (lumenna--settle)
  (lumenna-send (list :Answer (list :answer "text" :text date)))
  (lumenna--follow))

;;;###autoload
(defun lumenna-add-block ()
  "Add a block, once or repeating, on the day shown or today."
  (interactive)
  (unless (derived-mode-p 'lumenna-day-mode) (lumenna-today))
  (lumenna-send (list :Block (list :New (list :start nil :minutes nil))))
  (lumenna--follow))

(defun lumenna-day-add-block ()
  "Add a block on the day shown; in free time, starting there."
  (interactive)
  (if (equal (plist-get (lumenna--row-here) :role) "free")
      (lumenna-act-kind '("add_block"))
    (lumenna-send (list :Block (list :New (list :start nil :minutes nil))))
    (lumenna--follow)))

;;;; The block form

;; The app's own form: which fields it shows, what a kind's settings are, and
;; what saving sends are its.  Here each field is asked in the minibuffer, in
;; the form's words: a new block's essentials one after another, a block
;; changed a field at a time.

(defconst lumenna--new-block-fields '("title" "date" "start" "minutes" "kind" "repeat")
  "What a new block is asked, in order; the rest keep the form's values.")

(defun lumenna--block-value (form key)
  "What FORM, the app's block form, holds for field KEY, as text."
  (if (equal key "date")
      (plist-get form :date)
    (let ((value (plist-get (plist-get form :values) (intern (concat ":" key)))))
      (cond ((eq value t) "true") ((eq value :json-false) "false") ((null value) "") (t (format "%s" value))))))

(defun lumenna--read-block-field (form field)
  "Read FIELD of FORM, the app's block form, in its words: its hint, then its name."
  (let* ((key (plist-get field :key))
         (now (lumenna--block-value form key))
         (prompt (lumenna--prompt (plist-get field :hint) (plist-get field :label)))
         (example (plist-get field :example)))
    (pcase (plist-get field :kind)
      ("toggle" (if (y-or-n-p (lumenna--prompt
                               (concat (format (lumenna-word :now_is) (plist-get field :label)
                                               (plist-get (aref (lumenna-words :yes_no) (if (equal now "true") 0 1)) :title))
                                       "?")))
                    "true" "false"))
      ("choice"
       (let* ((choices (mapcar (lambda (o) (cons (plist-get o :title) (plist-get o :id)))
                               (append (plist-get field :options) nil)))
              (default (car (rassoc now choices))))
         (cdr (assoc (completing-read (lumenna--with-default prompt default) choices nil t nil nil default) choices))))
      (_ (if-let* ((syntax (plist-get field :syntax)))
             (lumenna-read-line prompt syntax now nil example)
           (read-from-minibuffer prompt now nil nil nil (and example (not (string-empty-p example)) example)))))))

(defun lumenna--block-field (key value)
  "Set the block form's field KEY to VALUE, the app's way."
  (lumenna-send (list :Block (list :Field (list :key key :value value))))
  (lumenna--settle))

(defun lumenna--fill-block-form ()
  "Fill in the app's block form, then save it; C-g closes it.
A new block asks its essentials in turn; a block changed asks which field.
Refused, the core's reason is said and a field asked for again."
  (let* ((form (plist-get lumenna--view :block_form))
         (fields (append (plist-get form :fields) nil))
         (new (and (lumenna--true (plist-get form :adding)) (not (plist-get form :refusal)))))
    (condition-case nil
        (progn
          (if new
              (dolist (key lumenna--new-block-fields)
                (when-let* ((field (seq-find (lambda (f) (equal (plist-get f :key) key))
                                             (append (plist-get (plist-get lumenna--view :block_form) :fields) nil))))
                  (lumenna--block-field key (lumenna--read-block-field (plist-get lumenna--view :block_form) field))))
            (let* ((choices (mapcar (lambda (f) (cons (plist-get f :label) f)) fields))
                   (field (cdr (assoc (completing-read (lumenna--prompt (plist-get form :refusal) (plist-get form :heading) (lumenna-word :change))
                                                       choices nil t)
                                      choices))))
              (lumenna--block-field (plist-get field :key) (lumenna--read-block-field form field))))
          (lumenna-send (list :Block "Save"))
          (lumenna--settle)
          (when-let* ((refused (plist-get (plist-get lumenna--view :block_form) :refusal)))
            (message "%s" refused)))
      (quit (lumenna-send (list :Block "Cancel"))
            (lumenna--settle)
            (signal 'quit nil)))))

(setq lumenna--block-form-function #'lumenna--fill-block-form)

;;;; Every series

(define-derived-mode lumenna-blocks-mode lumenna-list-mode "Lumenna Blocks"
  "Every block series.  RET or e changes every occurrence; d deletes one.

\\{lumenna-blocks-mode-map}"
  (lumenna--mode-name '(command "go-blocks"))
  (setq-local lumenna--draw #'lumenna--list-rows)
  (setq-local lumenna--activate (lambda (_row) (lumenna-act-edit))))

;;;###autoload
(defun lumenna-blocks ()
  "Show every block series, including ones on no day near today."
  (interactive)
  (lumenna-open-place "Blocks"))

;;;; Menus

(lumenna-define-keys lumenna-day-mode
  ((word :the_day)
   ("a" (command "new-block") lumenna-day-add-block)
   ("[" (command "previous-day") lumenna-day-previous)
   ("]" (command "next-day") lumenna-day-next)
   ("t" (command "go-to-now") lumenna-day-today)
   ("j" (command "go-to-day") lumenna-day-go-to)
   ("P" (command "plan-day") lumenna-day-plan)
   ("r" (command "replan") lumenna-day-replan)
   ("w" (command "what-now") lumenna-day-what-now)
   ("E" (command "end-day") lumenna-day-end)
   ("C" (command "clear-day") lumenna-day-clear))
  (:a_block
   ("e" (kinds "edit" "edit_task") lumenna-act-edit)
   ("i" (kinds "assign_task") lumenna-act-assign)
   ("x" (kinds "cancel_day") lumenna-act-cancel-day)
   ("o" (kinds "restore" "restore_day") lumenna-act-restore)
   ("R" (kinds "reminders") lumenna-act-reminders)
   ("N" (kinds "end_now") lumenna-act-end-now)
   ("+" (kinds "extend") lumenna-act-extend)
   ("f" (kinds "carry_forward" "carry_rest") lumenna-act-carry)
   ("d" (kinds "delete" "unassign") lumenna-act-delete))
  (:a_task_in_a_block
   ("RET" :the_task_itself lumenna-activate)
   ("s" (kinds "start_timer" "pause_timer" "resume_timer") lumenna-act-timer)
   ("S" (kinds "stop_timer") lumenna-act-stop-timer)
   ("l" (kinds "planned_length") lumenna-act-planned-length)
   ("m" (kinds "log_minutes") lumenna-act-log-minutes)))

(lumenna-define-keys lumenna-blocks-mode
  (:block_at_point
   ("e" (kinds "edit") lumenna-act-edit)
   ("R" (kinds "reminders") lumenna-act-reminders)
   ("d" (kinds "delete") lumenna-act-delete)))

(provide 'lumenna-day)

;;; lumenna-day.el ends here
