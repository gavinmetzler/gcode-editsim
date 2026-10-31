# Wishlist

Ideas we want to keep but have **not** put in the development plan yet.
Nothing here is scheduled or promised. When an item is picked up, it moves
into a milestone plan (`docs/plans/`) and ARCHITECTURE.md §13, and is
removed from this list.

Add an entry with the date, who raised it, and why it matters. Keep each
one short; the design happens when it is planned.

## Settings dialog

*Raised by the owner, 2026-10-31.*

A dialog where the user sets the program's options, kept between sessions.
The first setting it would hold:

- **When the toolpath updates after an edit:**
  - *on refresh only* (the default since M2.8a): edits mark the view out of
    date, with a red border and a **Refresh (F5)** button, which suits
    large programs and big edits;
  - *automatically*, after a short pause in typing (how M2.6 worked),
    which suits small programs and fast computers.

Other options that could live there (to be decided when it is planned):
arc tolerance, block delete, the dialect's decimal-point setting, colours,
and the default machine and dialect.

## Remember the last machine and dialect

*Raised by the owner, 2026-10-31.*

The app starts on the first machine in the list (the generic VMC) every
time, so a router program opened straight away is drawn mostly red
(outside that machine's travel). Remember the machine and dialect chosen
last, and start with them. Fits the settings dialog above, which would
keep its options between sessions in the same way.
