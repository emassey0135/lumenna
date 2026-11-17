# Module orbit-sdk

An unofficial Android library for apps that speak for themselves on Orbit Research's Android
devices, the Orbit Speak (braille keys) and the Orbit Player (a phone keypad). Start with
`OrbitSession` and `OrbitUi`; the README has a quick start, and `docs/DEVICE.md` what the devices
do that an app must fit.

# Package dev.emassey0135.orbit.session

`OrbitSession`: the device for one activity, handed the activity's lifecycle.

# Package dev.emassey0135.orbit.ui

The screen stack: `OrbitUi` and its asks (`list`, `menu`, `askLine`, `confirm`, `choose`, `form`),
the `Item`s, `Action`s, `MenuItem`s and `Field`s they take, and `Screen` and `ListScreen` for
screens of an app's own. The same shape as the BrailleSense SDK's `BrailleSenseUi`.

# Package dev.emassey0135.orbit

The pieces underneath, for an app that draws its screens its own way: `OrbitReader` (Orbit's
screen reader), `KeyDecoder` and `Key`, `EditBox`, `OrbitMenu`, `OrbitList`, `ListFinder`,
`OrbitDialog`, `BrailleInput` and `OrbitSettings`, `Claim` and `KeyDelivery`, `DebugKeys`, and the
device's own words (`OrbitWords`, `CellNames`).
