// Replan from Now and Plan the Day: the core's proposal for the day, said in full, then made as
// the person checks it.
//
// Every word is the core's (`proposal_words`): the screen's title, each check box's name, the
// buttons, the hint beside the apply button, what is shown when nothing is proposed, the heading
// over what does not fit. On opening it says the proposal's announcement, which is the whole
// proposal in one sentence. Each change is a line with a check box beside it, checked to begin
// with, named by the core's `item` and described by its line, as a reminder's buttons are
// described by its text. A change made alone can leave the day crowded, so after a partial
// apply the core proposes again around what was left, and that is what the screen shows next;
// it closes once nothing is left to propose. A refusal (the day moved since) is shown in the
// screen, which asks for a new proposal. Replan from Now's other options (skipping a block,
// each with what it costs) are buttons under the core's heading, each named by its sentence;
// choosing one asks again with its pins and shows that proposal in place.

import { useEffect, useRef, useState } from "react";
import { Button, Checkbox, Dialog, Heading, Modal } from "react-aria-components";
import { core } from "./core";
import type { DayPins, ReplanPins } from "./core/lumenna_web.js";
import type { ProposalWords, Proposed } from "./core";
import { say } from "./say";

/** Which screen: Replan from Now, or Plan the Day for a date. */
export type ProposalKind = { kind: "replan" } | { kind: "plan_day"; date: string };

interface Request {
  of: ProposalKind;
  /** Called when the screen closes: whether anything was made. */
  done: (changed: boolean) => void;
}

let open: ((request: Request) => void) | undefined;

/** Shows a proposal's screen until closed; resolves to whether anything was changed. */
export function showProposal(of: ProposalKind): Promise<boolean> {
  return new Promise((done) => {
    if (open) open({ of, done });
    else done(false);
  });
}

/** Where a proposal's screen is shown: rendered once, in the app. */
export function Proposals(props: { onChanged: () => void }) {
  const [request, setRequest] = useState<Request | undefined>();
  const changed = useRef(false);

  useEffect(() => {
    open = (next) => {
      changed.current = false;
      setRequest(next);
    };
    return () => {
      open = undefined;
    };
  }, []);

  const finish = () => {
    request?.done(changed.current);
    setRequest(undefined);
  };

  return (
    <Modal isDismissable isOpen={request !== undefined} onOpenChange={(isOpen) => !isOpen && finish()}>
      {request && (
        <Screen
          of={request.of}
          close={finish}
          onChanged={() => {
            changed.current = true;
            props.onChanged();
          }}
        />
      )}
    </Modal>
  );
}

function Screen(props: { of: ProposalKind; close: () => void; onChanged: () => void }) {
  const [words, setWords] = useState<ProposalWords | undefined>();
  const [proposal, setProposal] = useState<Proposed | undefined>();
  const [checked, setChecked] = useState<boolean[]>([]);
  const [problem, setProblem] = useState<string | undefined>();
  const [busy, setBusy] = useState(false);
  // Which proposal this is, counted: a new one is a new list, whose first check box takes focus.
  const [shown, setShown] = useState(0);

  // A proposal shown: every change checked, focus on the first, and the whole of it said.
  const show = (next: Proposed, said?: string) => {
    setProposal(next);
    setChecked(next.items.map(() => true));
    setShown((n) => n + 1);
    say(said ?? next.announcement);
  };

  const propose = (pins?: Proposed["pins"]): Promise<Proposed> =>
    props.of.kind === "replan"
      ? core.replan(pins as ReplanPins | undefined)
      : core.proposeDayPlan(props.of.date, pins as DayPins | undefined);

  useEffect(() => {
    let current = true;
    void Promise.all([core.proposalWords(props.of.kind), propose()]).then(
      ([words, next]) => {
        if (!current) return;
        setWords(words);
        show(next);
      },
      (error: Error) => {
        if (!current) return;
        setProblem(error.message);
        say(error.message);
      },
    );
    return () => {
      current = false;
    };
  }, []);

  const apply = async () => {
    if (!proposal || busy) return;
    setBusy(true);
    try {
      const accepted = proposal.items.flatMap((_, index) => (checked[index] ? [index] : []));
      const made =
        props.of.kind === "replan"
          ? await core.applyReplan(proposal, accepted)
          : await core.applyDayPlan(proposal, accepted);
      setProblem(undefined);
      props.onChanged();
      if (made.next.items.length === 0) {
        say(made.said);
        props.close();
      } else {
        show(made.next, made.said);
      }
    } catch (error) {
      // The day moved since: say so here, and propose again from the day as it is now.
      const message = (error as Error).message;
      setProblem(message);
      try {
        const next = await propose(proposal.pins);
        setProposal(next);
        setChecked(next.items.map(() => true));
      } catch {
        // The refusal is already shown.
      }
      say(message);
    } finally {
      setBusy(false);
    }
  };

  // Another option taken: the proposal it makes, shown and said in place of this one.
  const take = async (pins: ReplanPins) => {
    if (busy) return;
    setBusy(true);
    try {
      setProblem(undefined);
      show(await propose(pins));
    } catch (error) {
      setProblem((error as Error).message);
      say((error as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const nothing = proposal !== undefined && proposal.items.length === 0;

  return (
    <Dialog aria-labelledby="proposal-heading" className="proposal">
      <Heading slot="title" id="proposal-heading">
        {words?.title ?? ""}
      </Heading>
      {problem && <p role="alert">{problem}</p>}
      {nothing && words && <p>{words.nothing}</p>}
      {proposal && proposal.items.length > 0 && words && (
        <ul key={shown} className="plain changes" aria-labelledby="proposal-heading">
          {proposal.items.map((item, index) => (
            <li key={`${index}:${item.sentence}`}>
              <span id={`change-${index}`}>{item.sentence}</span>
              {item.reason && (
                <span id={`change-reason-${index}`} className="quiet">
                  {item.reason}
                </span>
              )}
              <Checkbox
                className="check"
                autoFocus={index === 0}
                isSelected={checked[index] ?? true}
                onChange={(on) => setChecked((all) => all.map((was, at) => (at === index ? on : was)))}
                aria-describedby={item.reason ? `change-${index} change-reason-${index}` : `change-${index}`}
              >
                <span className="box" aria-hidden="true" />
                {words.item}
              </Checkbox>
            </li>
          ))}
        </ul>
      )}
      {proposal && proposal.unfit.length > 0 && words && (
        <section aria-labelledby="proposal-unfit">
          <h3 id="proposal-unfit">{words.unfit}</h3>
          <ul className="plain">
            {proposal.unfit.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </section>
      )}
      {proposal && proposal.alternatives.length > 0 && words?.others && (
        <section aria-labelledby="proposal-others">
          <h3 id="proposal-others">{words.others}</h3>
          <ul className="plain">
            {proposal.alternatives.map((other) => (
              <li key={other.sentence}>
                <Button isDisabled={busy} onPress={() => void take(other.pins)}>
                  {other.sentence}
                </Button>
              </li>
            ))}
          </ul>
        </section>
      )}
      <div className="buttons">
        {/* With nothing proposed, the only thing to do: it takes focus. */}
        <Button key={nothing ? "only" : "cancel"} autoFocus={nothing} onPress={props.close}>
          {words?.cancel ?? ""}
        </Button>
        {proposal && !nothing && words && (
          <>
            <Button isDisabled={busy} aria-describedby="proposal-hint" onPress={() => void apply()}>
              {words.apply}
            </Button>
            <span id="proposal-hint" className="quiet">
              {words.hint}
            </span>
          </>
        )}
      </div>
    </Dialog>
  );
}
