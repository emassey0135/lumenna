;;; lumenna-day.el --- Lumenna's day planner and blocks -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Elijah Massey
;; SPDX-License-Identifier: MPL-2.0

;;; Commentary:

;; The day as it is lived (§13): blocks in time order with their sittings
;; under them, free time and now as lines of their own, and the blocks
;; cancelled for the day so they can be put back.  Opening on today puts point
;; on now.  Also the list of every block series, and the block form.

;;; Code:

(require 'lumenna)

(declare-function lumenna-task-show "lumenna-tasks")
(declare-function lumenna--choose-task "lumenna-tasks")

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
           ;; The core words the details for every app (§13).
           (let ((state (append (plist-get block :details) nil)))
             (push (list :id (plist-get block :id) :role "block" :block block :when (plist-get block :when)
                         :title (format "%s to %s, %s" (plist-get block :start) (plist-get block :end)
                                        (plist-get block :title))
                         :state (vconcat state))
                   rows)
             (dolist (sitting (append (plist-get block :assignments) nil))
               (let ((state (append (plist-get sitting :details) nil)))
                 (push (list :id (plist-get sitting :id) :role "assignment" :depth 1 :sitting sitting
                             :task (plist-get sitting :task) :block block :title (plist-get sitting :title)
                             :state (vconcat state)
                             :face (and (lumenna--true (plist-get sitting :running)) 'lumenna-running))
                       rows))))))
        ("free"
         (push (list :id (format "free@%s" (plist-get item :start)) :role "free" :free item :face 'lumenna-quiet
                     :title (format "Free, %s" (lumenna--length (plist-get item :minutes)))
                     :value (format "%s to %s" (plist-get item :start) (plist-get item :end)))
               rows))
        ("now"
         (push (list :id "now" :role "now" :face 'lumenna-now :title (format "Now, %s" (plist-get item :time)))
               rows))))
    (dolist (cancelled (append (plist-get plan :cancelled) nil))
      (push (list :id (format "cancelled@%s" (plist-get cancelled :series)) :role "cancelled" :cancelled cancelled
                  :face 'lumenna-quiet :title (format "%s, %s" (plist-get cancelled :start) (plist-get cancelled :title))
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
  "Show today, with point on now (§13)."
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

(defun lumenna--spoken-day (iso)
  "ISO as a person says it: `Sunday 4 October 2026'."
  (let ((time (encode-time (append '(0 0 12) (reverse (mapcar #'string-to-number (split-string iso "-")))))))
    (format-time-string "%A %-d %B %Y" time)))

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

(defun lumenna--day-row (&rest roles)
  "The row at point, which has to be one of ROLES."
  (let ((row (lumenna-row)))
    (unless (member (plist-get row :role) roles)
      (user-error "Not on a %s" (string-join roles " or ")))
    row))

(defun lumenna--day-activate (row)
  "RET on ROW: edit a block, open a sitting's task, put a cancelled day back."
  (pcase (plist-get row :role)
    ("block" (lumenna-day-edit-block))
    ("assignment" (lumenna-task-show row))
    ("free" (let ((free (plist-get row :free)))
              (lumenna-add-block lumenna--date (plist-get free :start) (min (plist-get free :minutes) 720))))
    ("cancelled" (lumenna-day-put-back))
    (_ (message "%s" (lumenna-describe row)))))

;;;; Blocks in the day

(defun lumenna-day-add-block ()
  "Add a block on the day shown; in free time, starting there."
  (interactive)
  (let ((row (get-text-property (line-beginning-position) 'lumenna-row)))
    (if (equal (plist-get row :role) "free")
        (lumenna--day-activate row)
      (lumenna-add-block lumenna--date))))

(defun lumenna-day-edit-block ()
  "Change the block at point.
A repeating one asks: this day only, or every one (§4.3)."
  (interactive)
  (let* ((block (plist-get (lumenna--day-row "block") :block))
         (series (plist-get block :series)))
    (if (and (lumenna--true (plist-get block :repeats))
             (equal (completing-read "Change which? " '("This day only" "Every occurrence") nil t)
                    "This day only"))
        (lumenna--edit-block series (lumenna-call "block.show" :id series) (list :date lumenna--date) block)
      (lumenna-edit-series series))))

(defun lumenna-day-assign ()
  "Put a task in the work block at point, for a sitting of the length chosen."
  (interactive)
  (let ((block (plist-get (lumenna--day-row "block") :block)))
    (unless (lumenna--true (plist-get block :accepts_tasks)) (user-error "That block takes no tasks"))
    (lumenna--assign (lumenna--choose-task (format "Assign to %s: " (plist-get block :title)))
                     (plist-get block :id) lumenna--date)))

(defun lumenna-day-cancel ()
  "Cancel the repeating block at point for this day alone."
  (interactive)
  (let ((block (plist-get (lumenna--day-row "block") :block)))
    (lumenna-write "block.cancel" :id (plist-get block :series) :date lumenna--date)))

(defun lumenna-day-put-back ()
  "Put this day of the block at point back as its series has it."
  (interactive)
  (let ((row (lumenna--day-row "block" "cancelled")))
    (lumenna-write "block.restore" :date lumenna--date
                   :id (plist-get (or (plist-get row :block) (plist-get row :cancelled)) :series))))

(defun lumenna-day-delete ()
  "Delete the block at point, or take the task at point out of its block."
  (interactive)
  (let ((row (lumenna--day-row "block" "assignment")))
    (if (equal (plist-get row :role) "assignment")
        (lumenna-write "unassign" :assignment (plist-get row :id))
      (let ((block (plist-get row :block)))
        (lumenna-delete-block (plist-get block :series) (plist-get block :title)
                              (lumenna--true (plist-get block :repeats)))))))

;;;; Sittings

(defun lumenna-day-timer ()
  "Start the timer on the sitting at point, pause it while it runs, or resume it.
Pausing keeps the time so far and leaves the sitting in progress (§3.7)."
  (interactive)
  (let ((sitting (plist-get (lumenna--day-row "assignment") :sitting)))
    (lumenna-write (if (lumenna--true (plist-get sitting :running)) "pause" "start")
                   :assignment (plist-get sitting :id))))

(defun lumenna-day-stop ()
  "Stop the timer on the sitting at point, ending the sitting, running or paused."
  (interactive)
  (let ((sitting (plist-get (lumenna--day-row "assignment") :sitting)))
    (lumenna-write "stop" :assignment (plist-get sitting :id))))

(defun lumenna--read-length (prompt &optional current)
  "Minutes for a sitting's planned length, or nil for none, read with PROMPT.
CURRENT, the length it has now, is offered to edit."
  (let ((text (string-trim (read-string (format "%s, in minutes, empty for none: " prompt)
                                        (and current (number-to-string current))))))
    (cond ((string-empty-p text) nil)
          ((and (string-match-p "\\`[0-9]+\\'" text) (> (string-to-number text) 0)) (string-to-number text))
          (t (user-error "That is not a number of minutes")))))

(defun lumenna-day-planned-length ()
  "Set how long the sitting at point is meant to take, or clear it (§3.7)."
  (interactive)
  (let* ((sitting (plist-get (lumenna--day-row "assignment") :sitting))
         (minutes (lumenna--read-length (format "Planned length of %s" (plist-get sitting :title))
                                        (plist-get sitting :planned_mins))))
    ;; No minutes is how "length" is told to clear it.
    (lumenna-write "length" :assignment (plist-get sitting :id) :minutes minutes)))

(defun lumenna-day-log-minutes (minutes)
  "Record MINUTES as the whole of the sitting at point, replacing what was logged."
  (interactive (list (read-number "Minutes, the whole of this sitting: ")))
  (lumenna-write "stop" :assignment (plist-get (plist-get (lumenna--day-row "assignment") :sitting) :id)
                 :minutes minutes))

(defun lumenna--assign (task block date)
  "Put TASK in BLOCK on DATE, asking how long the sitting is meant to take."
  (lumenna-write "assign" :task task :block block :date date
                 :minutes (lumenna--read-length "How long is this sitting meant to take")))

(defun lumenna-assign-task (task)
  "Put TASK in a work block of the coming week.
Which blocks those are is the core's (`block.choices'), as for every app."
  (let* ((found (lumenna-call "block.choices"))
         (choices (mapcar (lambda (block)
                            (cons (format "%s, %s to %s, %s"
                                          (lumenna--spoken-day (plist-get block :date))
                                          (plist-get block :start) (plist-get block :end)
                                          (plist-get block :title))
                                  (cons (plist-get block :id) (plist-get block :date))))
                          (append (plist-get found :blocks) nil))))
    (unless choices (user-error "There are no work blocks this week; add one in the day"))
    (let ((chosen (cdr (assoc (completing-read "Put it in: " choices nil t) choices))))
      (lumenna--assign task (car chosen) (cdr chosen)))))

;;;; Blocks: the form, and every series

(defun lumenna--read-kind (&optional current)
  "A block's kind, chosen by name, starting from CURRENT."
  (let ((choices '(("Work, takes tasks" . "work") ("Break" . "break") ("Event" . "event"))))
    (cdr (assoc (completing-read "Kind: " choices nil t nil nil (car (rassoc (or current "work") choices)))
                choices))))

(defun lumenna-add-block (&optional date at minutes)
  "Add a block, once or repeating, starting on DATE at AT for MINUTES."
  (interactive)
  (let* ((title (read-string "Name: "))
         (_ (when (string-blank-p title) (user-error "A block needs a name")))
         (at (read-string "Starts at, such as 9am or 14:30: " (or at "9am")))
         (minutes (read-number "Minutes: " (or minutes 60)))
         (kind (lumenna--read-kind))
         (date (read-string "Starting on: " (or date "today")))
         (repeat (string-trim (read-string "Repeats, such as every weekday, empty for once: "))))
    (lumenna-write "block.add" :title title :at at :minutes minutes :kind kind :date date
                   :repeat (unless (string-empty-p repeat) repeat))))

(defun lumenna-edit-series (series)
  "Change every occurrence of block SERIES, a field at a time."
  (lumenna--edit-block series (lumenna-call "block.show" :id series) (list :all t)))

(defconst lumenna--block-fields
  '(("Name" . title) ("Starts" . at) ("Lasts" . minutes) ("Kind" . kind) ("Repeats" . repeat)
    ("Notes" . notes) ("Takes tasks" . accepts_tasks) ("Counts toward capacity" . counts_capacity)
    ("Anchored" . anchored) ("Shortest length" . min_minutes) ("Tasks from" . task_filter)
    ("Until" . until) ("Colour" . colour))
  "A block's fields, by the name they are chosen by.")

(defconst lumenna--day-fields '(title at minutes kind accepts_tasks counts_capacity anchored)
  "What one day of a repeating block can change: what an exception holds (§3.6).")

(defun lumenna--edit-block (series shown scope &optional day)
  "Change one field of block SERIES, chosen by name, sending only that field.
SHOWN is the series as `block.show' gives it; DAY, for one day of it, is that
day's block from the plan, whose values are the day's.  SCOPE is the plist
saying which occurrences."
  (let* ((fields (if day
                     (seq-filter (lambda (f) (memq (cdr f) lumenna--day-fields)) lumenna--block-fields)
                   (seq-remove (lambda (f) (and (eq (cdr f) 'until) (not (lumenna--true (plist-get shown :repeats)))))
                               lumenna--block-fields)))
         (field (cdr (assoc (completing-read "Change: " fields nil t) fields)))
         (get (lambda (key) (if (and day (plist-member day key)) (plist-get day key) (plist-get shown key))))
         (value
          (pcase field
            ('title (read-string "Name: " (funcall get :title)))
            ('at (read-string "Starts at, such as 9am or 14:30: " (funcall get :start)))
            ('minutes (read-number "Lasts, in minutes: " (or (funcall get :duration_mins) (plist-get shown :minutes))))
            ('kind (lumenna--read-kind (funcall get :kind)))
            ('repeat
             (if (and (lumenna--true (plist-get shown :repeats)) (not (plist-get shown :repetition)))
                 (let ((typed (string-trim (read-string "Repeats by a rule this cannot show; type a new one, or none: "))))
                   (if (string-empty-p typed) (user-error "Nothing changed") typed))
               (let ((typed (string-trim (read-string "Repeats, empty to happen once: " (plist-get shown :repetition)))))
                 (if (string-empty-p typed) "none" typed))))
            ('notes (read-string "Notes, empty for none: " (plist-get shown :notes)))
            ((or 'accepts_tasks 'counts_capacity 'anchored)
             (let ((now (lumenna--true (funcall get (intern (format ":%s" field))))))
               (if (y-or-n-p (format "%s? It is %s now. " (car (rassq field fields)) (if now "yes" "no")))
                   t :json-false)))
            ('min_minutes (read-number "Shortest length in minutes, 0 for the kind's own: "
                                       (or (plist-get shown :min_minutes) 0)))
            ('task_filter (lumenna-read-line "Tasks from, a filter such as #Work, empty for any: " "filter"
                                             (plist-get shown :task_filter)))
            ('until (read-string "Last day, such as 31 January, or none: " (plist-get shown :until)))
            ('colour (read-string "Colour, such as teal, empty for none: " (plist-get shown :colour))))))
    (apply #'lumenna-write "block.edit" :id series (intern (format ":%s" field)) value scope)))

(defun lumenna-delete-block (series title repeats)
  "Delete block SERIES, called TITLE, asking first.
REPEATS says it is a series, so every occurrence goes."
  (when (yes-or-no-p (if repeats
                         (format "Every occurrence of %s goes, not only one day; to skip a day, cancel it instead.  Delete it? " title)
                       (format "Delete %s? It goes to the trash with its assignments. " title)))
    (lumenna-write "block.rm" :id series)))

(define-derived-mode lumenna-blocks-mode lumenna-list-mode "Lumenna Blocks"
  "Every block series.  RET or e changes every occurrence; d deletes one.

\\{lumenna-blocks-mode-map}"
  (setq-local lumenna--activate (lambda (row) (lumenna-edit-series (plist-get row :id)))))

;;;###autoload
(defun lumenna-blocks ()
  "Show every block series, including ones on no day near today (§3.6)."
  (interactive)
  (lumenna--show-list "*Lumenna: Blocks*" #'lumenna-blocks-mode
                      (lambda ()
                        (let ((listing (lumenna-call "block.list")))
                          (cons (format "Blocks, %s" (plist-get listing :announcement))
                                (append (plist-get listing :rows) nil))))))

(defun lumenna-blocks-edit ()
  "Change every occurrence of the block at point."
  (interactive)
  (lumenna-edit-series (plist-get (lumenna-row) :id)))

(defun lumenna-blocks-delete ()
  "Delete the block at point, asking first."
  (interactive)
  (let ((shown (lumenna-call "block.show" :id (plist-get (lumenna-row) :id))))
    (lumenna-delete-block (plist-get shown :id) (plist-get shown :title) (lumenna--true (plist-get shown :repeats)))))

;;;; Menus

(lumenna-define-keys lumenna-day-mode
  ("The day"
   ("a" "Add a block" lumenna-day-add-block)
   ("[" "Previous day" lumenna-day-previous)
   ("]" "Next day" lumenna-day-next)
   ("." "Today, at now" lumenna-day-today)
   ("j" "Go to a day" lumenna-day-go-to))
  ("A block"
   ("e" "Edit" lumenna-day-edit-block)
   ("i" "Assign a task" lumenna-day-assign)
   ("x" "Cancel this day" lumenna-day-cancel)
   ("o" "Put this day back" lumenna-day-put-back)
   ("d" "Delete the block, or take a task out of it" lumenna-day-delete))
  ("A task in a block"
   ("RET" "The task itself" lumenna-activate)
   ("s" "Start the timer, or pause or resume it" lumenna-day-timer)
   ("S" "Stop the timer, ending the sitting" lumenna-day-stop)
   ("l" "Planned length" lumenna-day-planned-length)
   ("m" "Log minutes by hand" lumenna-day-log-minutes)))

(lumenna-define-keys lumenna-blocks-mode
  ("The block at point"
   ("e" "Edit every occurrence" lumenna-blocks-edit)
   ("d" "Delete" lumenna-blocks-delete)))

(provide 'lumenna-day)

;;; lumenna-day.el ends here
