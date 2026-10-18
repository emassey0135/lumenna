# The planner's app round

What every client now shows of the planner. The core is on master (807d059, 80f5544,
b653ed8); CLAUDE.md's "Urgency and history" and the re-flow and day plan paragraphs say how
it behaves. **An app words nothing here itself**: every sentence, title and button is the
core's. Follow each platform's own conventions for where things go.

## 1. The day's own actions (`Plan::actions`)

The day's plan now carries actions of its own, for the day itself (subject `day`, target the
date). Offer them where the day is: the day's heading row, a Day menu, the day screen's
action list. In the core's order:

| Kind | Title | Question | What the app does |
|---|---|---|---|
| `plan_day` | Plan the Day | `form` | opens the Plan the Day screen (§3) for `action.target` |
| `replan` | Replan from Now | `form` | opens the Replan screen (§2) |
| `what_now` | What Should I Work On? | `pick` (title "What should I work on?", yes "Start") | the ordinary pick: `choices(action)`, then `act(action, picked)` |
| `end_day` | End the Day | `confirm` | ordinary |
| `clear_day` | Clear the Rest of the Day | `confirm` | ordinary |

Block and sitting rows also have new actions, all ordinary questions: End Now, Add 15 Minutes
(the block under way), Carry Forward (a short sitting, a pick), Carry the Rest Forward (a
block with more than one short sitting, `carry_rest`, immediate). A client whose rows already
run any action generically needs nothing for these; check that they appear and run.

## 2. Replan from Now

- `replan(style, ReplanPins::default(), None)` → `Replan { announcement, changes, unplaceable, pins, style }`.
  `style` is the platform's clock (`TimeStyle::TwelveHour`/`TwentyFourHour`, over RPC
  `"clock": "12-hour"|"24-hour"`).
- Words: `proposal_words(ActionKind::Replan)` (RPC `form.proposal {kind: "replan"}`):
  `title`, `item` (each change's check box name), `apply`, `cancel`, `hint`, `nothing`, `unfit`.
- The screen: say `announcement` when it opens (it is the whole proposal in one sentence).
  Then each change as a checkable row, its text `change.sentence`, all checked to begin with;
  `unplaceable` under the heading `unfit`, as plain lines; `hint` beside the apply button.
  With no changes, show `nothing` and no apply button.
- Apply: `apply_replan(proposal, checked, None)` → `Replanned { announcement, change, next }`.
  Say `announcement`. If `next.changes` is empty, close; otherwise **show `next` as the
  screen's new proposal**: a change taken alone can leave the day crowded, so the core
  proposes again around what was left unchecked. A refusal (the day moved since) is an error
  to show in the screen; ask `replan` again.
- Over RPC: `replan {clock, pins?}`, `replan.apply {proposal, accepted}`.

## 3. Plan the Day

- `propose_day_plan(Some(date), style, DayPins::default(), None)` → `DayPlan { announcement,
  date, sittings, unfit, pins, style }`; each sitting has `sentence` and an optional `reason`
  ("Suggested because …"): the reason is its row's description or second line.
- Words: `proposal_words(ActionKind::PlanDay)`.
- Apply: `apply_day_plan(plan, checked, None)` → `DayPlanned { announcement, change, next }`,
  as Replan: show `next` while it has sittings.
- Over RPC: `day_plan {date, clock, pins?}`, `day_plan.apply {plan, accepted}`.

## 4. A choice's reason

`Choice::reason` (optional) is why a choice is offered: "Suggested because overdue by 1 day,
priority 1, and its estimate of 30 minutes fits the 35 minutes left." Assign a Task and What
Should I Work On? give one with each choice. Show it as the choice's description: the
accessible description or hint, a second line, a subtitle. Never in its name.

## 5. A task's history

`TaskDetail::history` (lines: "3 sittings, 1 hour 30 minutes logged against a 1-hour
estimate", …) on the task's own screen only, under a "History" heading, never in a list.
`task_history(task)` (RPC `task.history`) adds the urgency and its terms if the screen wants
them; optional.

## 6. The carry offer at a block's end

`DueReminder::carry` is a button title ("Carry Forward") or empty. Where it is set, the
reminder's `text` already ends in the question. Give the notification that button beside
Dismiss and Snooze; it runs `carry_rest_forward("<target>@<occurrence>")` (RPC
`carry_rest {block}`) and says the change. Platforms that schedule ahead get the firing from
`upcoming_reminders` like any other, and re-arming already follows changes.

## 7. Settings

A new synced setting, `urgency-weights` (kind `line`, the core reads and refuses it). Apps
that list every synced setting get it for free; GTK and Windows place settings by key: put it
on the Planning page after the day window and week start.

## 8. Tests

- Each app tests what it adds: the day actions offered, the Replan screen saying its proposal
  and showing `next` after a partial apply, the Plan the Day screen, a choice's reason as a
  description, the history section, the carry button where notifications are testable.
- Tests must not depend on when they run. A test block of `00:00` for 1439 minutes ends at
  23:59 and is not under way in the day's last minute: use 1440. A test needing a block
  still to come today waits out the two minutes before midnight.

## Working on master

- Pull first (`git pull --rebase --autostash`), commit **by path**, only your app's directory
  (and its CLAUDE.md section), `git add` any new file first, then `git fetch && git push`
  (else `git pull --rebase --autostash` once and push).
- Never change `crates/`. If the core lacks something or a word is wrong, say so to
  `lumenna-f0` with SendMessage and carry on with the rest.
- Update your app's CLAUDE.md section for anything non-obvious.

## Second round (ffe43e9)

Two more, on the screens and notifications built in the first round.

### 9. Other options on the Replan screen

`Replan::alternatives` is a list (at most three) of `ReplanAlternative { sentence, pins }`:
"Skip Lunch today, your only break left today. Then nothing else needs to change." Show
them under the heading `proposal_words(replan).others` ("Other options"), each as a button
(or a row with an action) whose name is its `sentence`. Choosing one calls
`replan(style, alternative.pins, None)` and shows that proposal in place, saying its
`announcement`, as after a partial apply. A proposal with no alternatives shows no heading.
Plan the Day's `others` is empty: it has none.

### 10. The overrun warning's Replan button

A reminder can now be a block's work outrunning its time: "Deep work has 25 minutes left and
40 minutes of work planned, more than it holds." Its `DueReminder::replan` is a button title
("Replan") or empty. Where it is set, give the notification that button beside Dismiss and
Snooze; it opens the app at the Replan from Now screen (the app comes to the front where the
platform needs that). Where the app lists due reminders itself (web, Emacs, BTSpeak), offer it
there. Test what can be tested; say what cannot.
