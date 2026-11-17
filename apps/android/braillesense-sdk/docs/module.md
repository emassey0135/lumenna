# Module BrailleSense SDK

An unofficial Android library for apps that speak and write braille themselves on the HIMS
BrailleSense 7: its braille keys and QWERTY keyboard, speech and cells through its own services,
the person's Global Options, liblouis in their table, and lists, menus, edit boxes and dialogs that
read and move as the device's own.

Start with `dev.emassey0135.braillesense.ui.BrailleSenseUi`, the screen stack, over a
`dev.emassey0135.braillesense.session.BrailleSenseSession`. The README and `docs/DEVICE.md` in the
repository say what a device needs and how it behaves.

# Package dev.emassey0135.braillesense.ui

The screen stack (`BrailleSenseUi`) and what it is built from: the device's lists, menus, lines and
dialogs as `Frame`s, read as the device reads a control.

# Package dev.emassey0135.braillesense.session

An activity's wiring: the keys, speech and cells held only while the app is in front.

# Package dev.emassey0135.braillesense.input

The braille keys, the touch cursor and the snap-on QWERTY keyboard, as `Key`s named by the manual's
commands.

# Package dev.emassey0135.braillesense.output

Speech in the person's own voice and raw cells through the device's output service, with panning
and routing.

# Package dev.emassey0135.braillesense.braille

The person's Global Options, and liblouis in their table and grade.

# Package dev.emassey0135.braillesense.reading

How the device reads a control: its role's word and symbol, its letter, hot key and place, where the
person's settings put them.

# Package dev.emassey0135.braillesense.edit

An edit box as the device's own: held in cells as typed, with its keys, echo, selection and
clipboard.

# Package dev.emassey0135.braillesense.han

The device's service interfaces, under the vendor's descriptors.
