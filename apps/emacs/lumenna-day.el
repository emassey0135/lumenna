;;; lumenna-day.el --- Lumenna's day planner and blocks -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Elijah Massey
;; SPDX-License-Identifier: MPL-2.0

;;; Commentary:

;; The day as it is lived: blocks in time order with their sittings
;; under them, free time and now as lines of their own, and the blocks
;; cancelled for the day so they can be put back.  Opening on today puts point
;; on now.  Also the list of every block series, and the block form.

;;; Code:

(require 'lumenna)

(declare-function lumenna-task-show "lumenna-tasks" (id))

(defun lumenna--length (minutes)
  "MINUTES as words: `1 hour 30 minutes', as the other apps say it."
  (let ((hours (/ minutes 60)) (rest (% minutes 60)))
    (string-join
     (delq nil (list (and (> hours 0) (format "%d hour%s" hours (if (= hours 1) "" "s")))
                     (and (or (> rest 0) (= hours 0)) (format "%d minute%s" rest (if (= rest 1) "" "s")))))
     " ")))

(defun lumenna--day-rows (plan)
  "PLAN's timeline as rows, each said as the phone says it."
  (let ((blocks (append (plist-get plan :blocks) nil)) rows)
    (dolist (item (append (plist-get plan :timeline) nil))
      (pcase (plist-get item :item)
        ("block"
         (when-let* ((block (seq-find (lambda (b) (equal (plist-get b :row) (plist-get item :row))) blocks)))
           ;; The core words the details for every app.
           (let ((state (append (plist-get block :details) nil)))
             (push (list :id (plist-get block :id) :role "block" :block block :when (plist-get block :when)
                         :actions (plist-get block :actions)
                         :title (format "%s to %s, %s" (lumenna-time (plist-get block :start))
                                        (lumenna-time (plist-get block :end)) (plist-get block :title))
                         :state (vconcat state))
                   rows)
             (dolist (sitting (append (plist-get block :assignments) nil))
               (let ((state (append (plist-get sitting :details) nil)))
                 (push (list :id (plist-get sitting :id) :role "assignment" :depth 1 :sitting sitting
                             :actions (plist-get sitting :actions)
                             :task (plist-get sitting :task) :block block :title (plist-get sitting :title)
                             :state (vconcat state)
                             :face (and (lumenna--true (plist-get sitting :running)) 'lumenna-running))
                       rows))))))
        ("free"
         (push (list :id (format "free@%s" (plist-get item :start)) :role "free" :free item :face 'lumenna-quiet
                     :actions (plist-get item :actions)
                     :title (format "Free, %s" (lumenna--length (plist-get item :minutes)))
                     :value (format "%s to %s" (lumenna-time (plist-get item :start)) (lumenna-time (plist-get item :end))))
               rows))
        ("now"
         (push (list :id "now" :role "now" :face 'lumenna-now :title (format "Now, %s" (lumenna-time (plist-get item :time))))
               rows))))
    (dolist (cancelled (append (plist-get plan :cancelled) nil))
      (push (list :id (format "cancelled@%s" (plist-get cancelled :series)) :role "cancelled" :cancelled cancelled
                  :actions (plist-get cancelled :actions)
                  :face 'lumenna-quiet :title (format "%s, %s" (lumenna-time (plist-get cancelled :start)) (plist-get cancelled :title))
                  :state ["cancelled for this day"])
            rows))
    (nreverse rows)))

(defvar-local lumenna--day nil "The day shown, as a date phrase; nil follows today.")
(defvar-local lumenna--date nil "The ISO date the day buffer last showed.")

(define-derived-mode lumenna-day-mode lumenna-list-mode "Lumenna Day"
  "A day: blocks with their sittings under them, free time, and now.
RET edits a block, shows a sitting's task, adds a block in free time, or puts
a cancelled day back.  [ and ] move between days.

\\{lumenna-day-mode-map}"
  (setq-local lumenna--activate #'lumenna--day-activate))

;;;###autoload
(defun lumenna-today ()
  "Show today, with point on now."
  (interactive)
  (lumenna--show-list "*Lumenna: Day*" #'lumenna-day-mode #'lumenna--day-listing 'lumenna--day nil)
  (lumenna--go-to-now))

(defun lumenna--day-listing ()
  "The heading and rows of the day this buffer shows."
  (let ((plan (if lumenna--day (lumenna-call "plan" :date lumenna--day) (lumenna-call "plan"))))
    (setq lumenna--date (plist-get plan :date))
    (cons (format "%s, %s" (lumenna--spoken-day lumenna--date)
                  (let ((summary (plist-get plan :summary)))
                    (if (string-empty-p summary) (plist-get plan :announcement) summary)))
          (lumenna--day-rows plan))))

(defun lumenna--go-to-now ()
  "Point on now, or on the block happening now; else the first line of the day."
  (goto-char (point-min))
  (let (found)
    (while (and (not found) (zerop (forward-line 1)) (not (eobp)))
      (let ((row (get-text-property (point) 'lumenna-row)))
        (when (or (equal (plist-get row :role) "now") (equal (plist-get row :when) "now"))
          (setq found t))))
    (unless found
      (goto-char (point-min))
      (forward-line 1))))

(defun lumenna--day-turn (day)
  "Show DAY, a date phrase, or today for nil; and say it."
  (setq lumenna--day day)
  (lumenna-refresh)
  (goto-char (point-min))
  (forward-line 1)
  (message "%s" (buffer-substring-no-properties (point-min) (line-end-position 0))))

(defun lumenna--day-shift (days)
  "The date DAYS after the one this buffer shows, as YYYY-MM-DD."
  (let ((time (encode-time (append '(0 0 12) (reverse (mapcar #'string-to-number (split-string lumenna--date "-")))))))
    (format-time-string "%Y-%m-%d" (time-add time (* days 86400)))))

(defun lumenna-day-previous ()
  "Show the day before."
  (interactive)
  (lumenna--day-turn (lumenna--day-shift -1)))

(defun lumenna-day-next ()
  "Show the day after."
  (interactive)
  (lumenna--day-turn (lumenna--day-shift 1)))

(defun lumenna-day-today ()
  "Show today, with point on now."
  (interactive)
  (setq lumenna--day nil)
  (lumenna-refresh)
  (lumenna--go-to-now))

(defun lumenna-day-go-to (day)
  "Show DAY: any date phrase, such as \"next friday\" or \"2026-12-01\"."
  (interactive (list (read-string "Go to day: " nil nil "tomorrow")))
  (lumenna--day-turn day))

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

(defun lumenna--day-activate (row)
  "RET on ROW: edit a block, open a sitting's task, add a block in free time,
put a cancelled day back."
  (pcase (plist-get row :role)
    ("block" (lumenna-act-edit))
    ("assignment" (lumenna-act-edit))
    ("free" (lumenna-act-kind '("add_block")))
    ("cancelled" (lumenna-act-restore))
    (_ (message "%s" (lumenna-describe row)))))

(defun lumenna-day-add-block ()
  "Add a block on the day shown; in free time, starting there."
  (interactive)
  (if (equal (plist-get (lumenna--row-here) :role) "free")
      (lumenna-act-kind '("add_block"))
    (lumenna-add-block lumenna--date)))

;;;; The block form

;; A form the core leaves to the client, on the core's rules: the fields it
;; starts from (`form.block_fields', `form.day_block_fields'), a kind's own
;; settings (`form.block_defaults'), and what saving sends (`form.block_edit',
;; `form.new_block').

(defconst lumenna--block-fields
  '(("Name" . title) ("Starts" . start) ("Lasts" . minutes) ("Kind" . kind) ("Repeats" . repeat)
    ("Notes" . notes) ("Takes tasks" . accepts_tasks) ("Counts toward capacity" . counts_capacity)
    ("Anchored" . anchored) ("Shortest length" . min_minutes) ("Tasks from" . task_filter)
    ("Until" . until) ("Colour" . colour))
  "The block form's fields, by the name they are chosen by.")

(defconst lumenna--day-fields '(title start minutes kind accepts_tasks counts_capacity anchored)
  "The fields of the form for one day of a repeating block.")

(defconst lumenna--kinds '(("Work, takes tasks" . "work") ("Break" . "break") ("Event" . "event"))
  "A block's kinds, by the name they are chosen by.")

(defun lumenna--read-kind (&optional current)
  "A block's kind, chosen by name, starting from CURRENT."
  (cdr (assoc (completing-read "Kind: " lumenna--kinds nil t nil nil (car (rassoc (or current "work") lumenna--kinds)))
              lumenna--kinds)))

(defun lumenna--with-kind (fields kind)
  "FIELDS of kind KIND, with its own settings.
As a form's check boxes go back to them when the kind changes."
  (let ((fields (plist-put (copy-sequence fields) :kind kind))
        (defaults (lumenna--value "form.block_defaults" :kind kind)))
    (dolist (flag '(:accepts_tasks :counts_capacity :anchored) fields)
      (setq fields (plist-put fields flag (plist-get defaults flag))))))

(defun lumenna--read-block-field (field fields)
  "FIELDS with FIELD changed, read in the minibuffer."
  (let* ((key (intern (format ":%s" field)))
         (now (plist-get fields key))
         (name (car (rassq field lumenna--block-fields))))
    (pcase field
      ('kind (lumenna--with-kind fields (lumenna--read-kind now)))
      ((or 'accepts_tasks 'counts_capacity 'anchored)
       (plist-put (copy-sequence fields) key
                  (if (y-or-n-p (format "%s? It is %s now. " name (if (lumenna--true now) "yes" "no"))) t :json-false)))
      (_ (plist-put (copy-sequence fields) key
                    (pcase field
                      ('start (read-string "Starts at, such as 9am or 14:30: " now))
                      ('minutes (read-string "Lasts, in minutes: " now))
                      ('repeat (read-string "Repeats, such as every weekday, empty to happen once: " now))
                      ('min_minutes (read-string "Shortest length in minutes, empty for the kind's own: " now))
                      ('task_filter (lumenna-read-line "Tasks from, a filter such as #Work, empty for any: " "filter" now))
                      ('until (read-string "Last day, such as 31 January, empty for for good: " now))
                      ('colour (read-string "Colour, such as teal, empty for none: " now))
                      ('notes (read-string "Notes, empty for none: " now))
                      (_ (read-string (format "%s: " name) now))))))))

(defun lumenna--edit-block (id before choices scope)
  "Change one of CHOICES of block ID, whose form starts from BEFORE.
Only what changed is sent; SCOPE is the plist saying which occurrences."
  (let* ((field (cdr (assoc (completing-read "Change: " choices nil t) choices)))
         (after (lumenna--read-block-field field before))
         (edit (lumenna--value "form.block_edit" :before before :after after)))
    (if edit
        (apply #'lumenna-write "block.edit" :id id (append edit scope))
      (message "Nothing changed"))))

(defun lumenna-edit-series (series)
  "Change every occurrence of block SERIES, a field at a time."
  (let* ((shown (lumenna-call "block.show" :id series))
         (choices (seq-remove (lambda (f) (and (eq (cdr f) 'until) (not (lumenna--true (plist-get shown :repeats)))))
                              lumenna--block-fields)))
    (lumenna--edit-block series (lumenna--value "form.block_fields" :block shown) choices '(:all t))))

(defun lumenna--edit-day-block (block date)
  "Change BLOCK, from the plan for DATE.
A repeating one asks: this day only, or every one."
  (if (and (lumenna--true (plist-get block :repeats))
           (equal (completing-read "Change which? " '("This day only" "Every occurrence") nil t)
                  "This day only"))
      (lumenna--edit-block (plist-get block :series) (lumenna--value "form.day_block_fields" :block block)
                           (seq-filter (lambda (f) (memq (cdr f) lumenna--day-fields)) lumenna--block-fields)
                           (list :date date))
    (lumenna-edit-series (plist-get block :series))))

(defun lumenna-add-block (&optional date start)
  "Add a block, once or repeating, starting on DATE at START."
  (interactive)
  (let* ((title (read-string "Name: "))
         (start (read-string "Starts at, such as 9am or 14:30: " (or start "9am")))
         (minutes (read-string "Lasts, in minutes: " "60"))
         (fields (lumenna--with-kind (list :title title :start start :minutes minutes) (lumenna--read-kind)))
         (date (read-string "Starting on: " (or date "today")))
         (fields (plist-put fields :repeat (read-string "Repeats, such as every weekday, empty for once: "))))
    (dolist (key '(:until :min_minutes :task_filter :colour :notes))
      (setq fields (plist-put fields key "")))
    (apply #'lumenna-write "block.add" (lumenna--value "form.new_block" :fields fields :date date))))

(lumenna-define-form "block" "edit"
                     (lambda (_action row)
                       (lumenna--edit-day-block (plist-get row :block) lumenna--date)))
(lumenna-define-form "series" "edit" (lambda (action _row) (lumenna-edit-series (plist-get action :target))))
(lumenna-define-form "free_time" "add_block"
                     (lambda (action _row) (lumenna-add-block (plist-get action :target) (plist-get action :other))))

;;;; Every series

(define-derived-mode lumenna-blocks-mode lumenna-list-mode "Lumenna Blocks"
  "Every block series.  RET or e changes every occurrence; d deletes one.

\\{lumenna-blocks-mode-map}"
  (setq-local lumenna--activate (lambda (_row) (lumenna-act-edit))))

;;;###autoload
(defun lumenna-blocks ()
  "Show every block series, including ones on no day near today."
  (interactive)
  (lumenna--show-list "*Lumenna: Blocks*" #'lumenna-blocks-mode
                      (lambda ()
                        (let ((listing (lumenna-call "block.list")))
                          (cons (format "Blocks, %s" (plist-get listing :announcement))
                                (append (plist-get listing :rows) nil))))))

;;;; Menus

(lumenna-define-keys lumenna-day-mode
  ("The day"
   ("a" "Add a block" lumenna-day-add-block)
   ("[" "Previous day" lumenna-day-previous)
   ("]" "Next day" lumenna-day-next)
   ("t" "Today, at now" lumenna-day-today)
   ("j" "Go to a day" lumenna-day-go-to))
  ("A block"
   ("e" "Edit Block, or a sitting's Edit Task Details" lumenna-act-edit)
   ("i" "Assign a Task" lumenna-act-assign)
   ("x" "Cancel This Day" lumenna-act-cancel-day)
   ("o" "Restore This Day" lumenna-act-restore)
   ("d" "Delete Block, or a sitting's Unassign" lumenna-act-delete))
  ("A task in a block"
   ("RET" "The task itself" lumenna-activate)
   ("s" "Start, Pause or Resume Timer" lumenna-act-timer)
   ("S" "Stop Timer" lumenna-act-stop-timer)
   ("l" "Planned Length" lumenna-act-planned-length)
   ("m" "Log Minutes" lumenna-act-log-minutes)))

(lumenna-define-keys lumenna-blocks-mode
  ("The block at point"
   ("e" "Edit Block" lumenna-act-edit)
   ("d" "Delete Block" lumenna-act-delete)))

(provide 'lumenna-day)

;;; lumenna-day.el ends here
