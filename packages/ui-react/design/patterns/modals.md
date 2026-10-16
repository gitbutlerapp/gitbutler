# Modals

**Three parts, in code and in ⚛️ Core.** `ModalHeader`, `ModalBody` and
`ModalFooter` inside `Modal`; in Figma, `Modal / Header`, `Modal / Body` and
`Modal / Footer` on the popup container. Their descriptions carry the rules
below that the code can hold; change one and change the other.

**Copy a whole one.** Modal's stories, after `Default`, are the
confirmation, the form and the destructive question these notes describe,
opened and written as the app writes them, and the Modal examples beside the
Modal section in ⚛️ Core draw the same three. Start from the closest and
change the words.

**Pick the kind before the layout.** A prompt, a confirmation or a short form
is built from the three parts. A pane that holds more than one group, like
settings or the conflict resolver, lays itself out. Nothing else goes in a
modal: a list to search and pick from is a `PickerDialog` (its
`CommandPalette` story), or, anchored to the control that opened it, Popup's
combobox (its `SelectProject` story).

**One task per modal.** A modal asks one thing and closes on the answer. It
may drill in one level: a list of choices whose pick opens that choice's step
in the same modal, with Back in the header (`onBack` on `ModalHeader`) to
return to the list — "Where does the code live?", then "Connect a computer".
If it runs a sequence instead (Next, Next, Finish), or a step opens another
list of its own, it is a page or a panel, not a modal.

**Size by what it holds, starting at `small`.** `xsmall` (320) for a one-line
question, `small` (420) for a prompt with a field or a short list, `medium`
(640) for a pane around one group, `large` (820) for settings, the conflict
resolver or any pane with more than one group. Before `large`, ask
whether the content wants a page. Height follows the content; only the body
scrolls.

**`alert` only when the question has to be answered.** Credentials, and
anything that destroys work. Every other modal closes on Escape and on the
backdrop, which is why none has a close button.

**The title asks the question or names the task; the description says what
happens.** "Discard 3 files?", then "Their changes are gone for good". The
answer button repeats the verb — Discard, Upload, Continue — never "OK" or
"Yes", so the button reads right without the title. See
[Voice](../content/voice.md).

**Cancel, then the answer.** Cancel is `ghost`; the answer is `gray`, or
`danger` when it destroys something, and sits last. Never `pop`: two buttons
are not the busy surface pop is kept for. See [Buttons](../components/buttons.md)
and [Emphasis](../foundations/emphasis.md).
