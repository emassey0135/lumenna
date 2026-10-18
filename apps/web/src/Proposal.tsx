// Replan from Now and Plan the Day: the app's proposal screen (crates/app), drawn.
//
// Every word is the core's (`proposal_words`): the screen's title, each check box's name, the
// buttons, the hint beside the apply button, what is shown when nothing is proposed, the heading
// over what does not fit. The proposal is said as it comes. Each change is a line with a check
// box beside it, checked to begin with, named by the core's `item` and described by its line, as
// a reminder's buttons are described by its text. After a partial apply the app shows the next
// proposal, a new list whose first box takes focus, and closes once nothing is left. A refusal
// is shown on the screen. Replan from Now's other options are buttons named by their sentences.

import { Button, Checkbox, Dialog, Heading, Modal } from "react-aria-components";
import type { Event, ProposalView } from "./core";
import { current, sayAll, send } from "./session";

/**
 * Opens Replan from Now from wherever it is asked for — an overrun warning — and resolves once it
 * closes: whether anything was made.
 */
export async function showReplan(): Promise<boolean> {
  const turn = await send({ Proposal: "Replan" });
  sayAll(turn.said);
  return new Promise((resolve) => {
    const wait = () =>
      void current().then((screen) => {
        if (screen.proposal) setTimeout(wait, 200);
        else resolve(screen.proposal_made);
      });
    setTimeout(wait, 200);
  });
}

/** Where a proposal's screen is shown: rendered once, in the app, while the app has one open. */
export function Proposals(props: { proposal: ProposalView | undefined; onChanged: () => void }) {
  const close = () => void send({ Proposal: "Close" });
  return (
    <Modal isDismissable isOpen={props.proposal !== undefined} onOpenChange={(isOpen) => !isOpen && close()}>
      {props.proposal && <Screen proposal={props.proposal} close={close} onChanged={props.onChanged} />}
    </Modal>
  );
}

function Screen(props: { proposal: ProposalView; close: () => void; onChanged: () => void }) {
  const { proposal } = props;
  const { words } = proposal;

  /** Runs `event` and says what it said: a proposal, what was made, or why not. */
  const act = async (event: Event) => {
    const turn = await send(event);
    sayAll(turn.said);
    props.onChanged();
  };

  const nothing = proposal.ready && proposal.changes.length === 0;

  return (
    <Dialog aria-labelledby="proposal-heading" className="react-aria-Dialog proposal">
      <Heading slot="title" id="proposal-heading">
        {words.title}
      </Heading>
      {proposal.refusal && <p role="alert">{proposal.refusal}</p>}
      {nothing && <p>{words.nothing}</p>}
      {proposal.changes.length > 0 && (
        <ul key={proposal.shown} className="plain changes" aria-labelledby="proposal-heading">
          {proposal.changes.map((change, index) => (
            <li key={`${index}:${change.sentence}`}>
              <span id={`change-${index}`}>{change.sentence}</span>
              {change.reason && (
                <span id={`change-reason-${index}`} className="quiet">
                  {change.reason}
                </span>
              )}
              <Checkbox
                className="check"
                autoFocus={index === 0}
                isSelected={change.checked}
                onChange={(on) => void send({ Proposal: { Check: { index, on } } })}
                aria-describedby={change.reason ? `change-${index} change-reason-${index}` : `change-${index}`}
              >
                <span className="box" aria-hidden="true" />
                {words.item}
              </Checkbox>
            </li>
          ))}
        </ul>
      )}
      {proposal.unfit.length > 0 && (
        <section aria-labelledby="proposal-unfit">
          <h3 id="proposal-unfit">{words.unfit}</h3>
          <ul className="plain">
            {proposal.unfit.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </section>
      )}
      {proposal.others.length > 0 && words.others && (
        <section aria-labelledby="proposal-others">
          <h3 id="proposal-others">{words.others}</h3>
          <ul className="plain">
            {proposal.others.map((sentence, index) => (
              <li key={sentence}>
                <Button isDisabled={proposal.busy} onPress={() => void act({ Proposal: { Take: index } })}>
                  {sentence}
                </Button>
              </li>
            ))}
          </ul>
        </section>
      )}
      <div className="buttons">
        {/* With nothing proposed, the only thing to do: it takes focus. */}
        <Button key={nothing ? "only" : "cancel"} autoFocus={nothing} onPress={props.close}>
          {words.cancel}
        </Button>
        {proposal.ready && !nothing && (
          <>
            <Button isDisabled={proposal.busy} aria-describedby="proposal-hint" onPress={() => void act({ Proposal: "Apply" })}>
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
