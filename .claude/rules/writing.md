# Writing

```text
Purpose:  How everything an agent writes for people to read is written.
Covers:   The record, code comments, commit messages, words in the app, instructions
          for agents, and replies in chat. A rule made for files, such as one sentence
          per line, holds only in files.
Loaded:   Into every session, by Claude Code, from .claude/rules/.
Source:   Adapted from Dual Input's .claude/rules/writing.md.
```

## The test

Can it be read once, at speed, and understood?
A sentence that needs a second pass has failed, however accurate it is.
A reader who has to work stops reading, and a reader who stops reading acts on half a document.

When a rule below disagrees with this test, the test wins.

## Kinds of document

Before the first sentence, know which kind you're writing.
The kind decides who reads it and what shape it takes.

| Kind                | Lives in                                | The reader wants to                                                                       |
| ------------------- | --------------------------------------- | ----------------------------------------------------------------------------------------- |
| Design              | `record/areas/<area>/NN-<slug>.md`      | know what a part is and why it's that way                                                 |
| Plan                | `record/areas/<area>/plan.md`           | know what's being built and where it stands                                               |
| Log                 | `record/areas/<area>/log.md`            | know what was found                                                                       |
| Decision            | `record/decisions/NNN-<slug>.md`        | know why one option won                                                                   |
| Tentative decisions | `record/areas/<area>/decisions.md`      | know what an area's experiments or design work have settled for now, and on what findings |
| Reference           | beside what it describes                | look one thing up                                                                         |
| Agent instructions  | `CLAUDE.md` and `.claude/`, this file included | act correctly without asking                                                       |

A document that's two kinds at once serves neither reader.
A design doc that also tracks progress goes stale in its progress half and takes the design half down with it.

### Design, plan, and log

These three have different lifetimes, and keeping them apart is what keeps each one true.

**A design doc** says what a part is, and it's stable.
When a result overturns part of it, add a blockquote at the top naming the log entry, and leave the text below as it was.
A design doc that quietly changed is worse than one that's openly out of date.
Once its notes overturn so much of the body that a reader can't tell what holds, rewrite it to its current state and drop the notes, since git keeps the old text and the log keeps why it changed.
It opens with three lines, so it can be judged before it's read:

```text
Purpose:     What this part is for, in one line.
Depends on:  The docs it assumes, or "nothing".
Decides:     The questions it settles.
```

**A plan** says what's being built, in stages, each with an exit criterion you can watch happen.
It's the only place project status lives.
An index or design doc that says "not built yet" is a second copy of the status, and it'll be the wrong one.

**A log** says what was found, newest entry first, one dated heading per entry: `## <YYYY-MM-DD>: <what was found>`.
Write the entry when the result arrives, not at the end of the day.
Each entry says what we asked, what we expected, what we found, what changed because of it, and which docs it affects.
A negative result gets the same care as a positive one, because what was tried and abandoned is the part git history can't tell anyone.
An experiment's raw output stays beside its probe, in `experiments/areas/<area>/`, and the area's log gets an entry only for a finding worth keeping.

### Decisions

One file per decision, numbered, written when the decision is made.
It gives the context that forced a choice, each option with what it would cost, the decision, and what it commits us to.
Once made, a decision isn't edited, except to add a blockquote at the top naming the decision that reverses it.
The reversing decision says what it reverses and links back.

When an area's experiments or design work settle a question before the design it belongs to is done, the answer goes in the area's `decisions.md` as a tentative decision, because a numbered decision can't be edited and these change as work goes on.
Each one gives what was decided, the findings behind it with links to where each was found, and what it leaves out, if anything, and it's edited in place as findings arrive.
The finished design confirms each one as a decision in `record/decisions/`, changes it, or drops it, and the tentative one links to the result.

### Reference

Tables and lists, with no argument.
The reasoning lives in a design doc, and the reference links to it.

## Structure

**Lead with the answer.**
The first paragraph says what the document establishes.
Background, history, and caveats come after the reader knows what they're for.
So no heading is called Overview, Introduction, Summary, or Conclusion: the first paragraph is the overview, and a closing summary repeats the document.

**One document, one job.**
A document runs as long as its job needs, and a second job goes into a document of its own.

**Write a value down once.**
Everywhere else links to it.
A value written twice is two values that haven't disagreed yet.
This holds for numbers, rules, status, and vocabulary alike.

**Describe the current state, not the edit.**
Don't write "now", "updated to", or "no longer", except in a log, an amendment blockquote, or a decision that reverses another.
Git holds the change, and a reader arriving next year doesn't care what the text used to say.

**Headings name the content.**
Write "Known limits", not "Limits, stated rather than discovered".
Use sentence case, and never a question.

**Procedures are numbered steps.**
Each step is one action and says what you'll see when it worked.
The condition comes before the action, "If the game is running, close it", so the reader doesn't act before reaching it.

**Delete an empty template section**, unless the emptiness is the information.
"No known traps" is worth a line.

**Links within the repository are relative, and every link's text names the target.**
Write `[01-spine](01-spine.md)`, never "here" or "this doc".

**Files are kebab-case.**
Use a numbered prefix (`01-`) only where order matters, and write dates as `YYYY-MM-DD`.

## Sentences

**One sentence per line** in the source, list items included.
A diff then shows the sentence that changed rather than the paragraph that reflowed, and a sentence that runs off the edge of the editor is visibly too long.
A table cell and a front-matter field are the exceptions, because neither can span lines.

**Show the mechanism, not a property of it.**
Naming a property makes the reader rebuild the concrete case to check the claim.
Showing the case carries the conclusion and costs fewer words.

```text
✗  Following keeps everyone's copy of a mod in sync.
✓  The sharer saves a file in the mod's folder, the host passes it on to every
   follower, and each follower's copy is replaced with the new file.
```

**Be concrete.**
Give a name, a number, a version, or a path.
"Some mods" is a claim nobody can check, and naming the mods makes it one they can.

**Never invent a specific.**
Every name, number, version, API, and path is real, or is labeled hypothetical where it appears.
The push to be concrete is exactly what makes a plausible fabrication tempting, and a made-up file name in the record costs a test session with friends to disprove.

**Main clause first.**
A sentence that opens with three conditions makes the reader hold all three before learning what they're conditions on.
Procedures and agent instructions are the exception: there the condition comes first.

**Name who does it.**
Write "the host drops the frame", not "the frame is dropped".
The passive hides the fact a debugging reader needs most: which component to blame.

**Say it once.**
A restatement three paragraphs later, dressed to feel like it adds something, is the same sentence twice.
Before calling a paragraph done, ask what the reader loses if it goes.
If the answer is nothing, it goes.

**Cut the trailing clause that restates the sentence.**
The pattern is an em dash, then a clause that sounds like an explanation and repeats the sentence in a more pleased-with-itself register.

```text
✗  The follower's copy is replaced by renaming over it — which is what a careful
   installer would do.
```

"Which is what X would do", where X does no such thing, is the commonest form.

**No rhetoric.**
Don't argue against a doubt nobody raised: "not a matter of trying harder", "not just a log collector".
Don't ask rhetorical questions.
Don't build suspense: "The catch?" and "The result: nothing" hold back the fact the reader came for.

**One word for one thing.**
Define a term at its first use in each document, then use exactly that term.
Switching between "tester", "guest", and "the other player" for variety makes the reader wonder whether they're three things.
Expand an acronym at its first use in each document, unless every reader of this repository already knows it: API, UI, CI, DLL, and the game's own, like CET and RED4ext.

**Use modal verbs precisely.**
Must is required, should is recommended, and may is optional.
State expected behavior as fact, "the copy updates", because "the copy should update" can't tell a requirement from a prediction.

**Use an analogy only if it predicts correctly**, and say where it stops holding.

**Tense follows time.**
Use the present for how things work, the past for what we did, and the future for plans and consequences.

## Evidence

A design built on a wrong claim fails in a session with friends weeks later, far from the sentence that caused it.
So every claim about the game, Windows, Linux, Proton, or a library, and any other claim a reader could ask "says who?" about, says where it came from:

- **Measured:** we ran it.
  Name the platform, the date, and the log entry: `(measured on Windows 11, <date>, [log](log.md#<entry>))`.
  For the game, name its version: `(measured on Cyberpunk 2077 2.31, <date>, ...)`.
- **Documented:** a vendor or project says so.
  Link the page.
  Before a design leans on a documented claim, we measure it.
- **Read:** we read it in code or files, ours or someone else's.
  Name the project, its version or commit, and the file: `(read in CP77 Log Collector 1.3.1, src/Mods.cs)`.
  Code says what would happen, not that it runs that path, so a claim that turns on which path runs stays unverified until measured.
- **Unverified:** an inference, a forum post, or a guess.
  Mark it unverified, and if anything depends on it, say what would settle it.

For a source in another of our repositories, cite the repository, the path, and for a log the entry heading, as text, because a link would break for anyone else: `(read in CP77 Log Collector, README.md, "Sharing mods")`.

A claim with no marker is knowledge a working developer accepts without a source, like "a rename within one folder replaces the target in one step".

Say what we don't know.
A gap that's stated can be closed, and a gap that's skipped is a trap.
A "TBD" says what's unknown and what would settle it.

Quote exactly.
Copy error messages, log lines, and identifiers verbatim, because the reader will search for them.

Code in a document runs as shown, or is labeled a sketch.
Check function and API names against the crate's docs or source before they go in.

## Units, identifiers, and numbers

- Every quantity carries its unit.
- Measurements, and anything a reader might look up or compare, are digits: 3,000 words, 150 lines, 20ms.
  A count under 10 in running prose can be a word, limits included: two players, one em dash.
- Time units close up with no space: 20ms, 2s.
  Sizes take a space: 32 MB, 256 KB.
- Printed counts handle the singular: "1 file", never "1 files".
- Dates are absolute, like 2026-11-13.
  A fact about the world that's true only for now, such as how a game version behaves, says "as of" and a date.
  Project status goes in the plan instead.
- Versions are specific: "Cyberpunk 2077 2.31", never "the latest patch".
- Paths in the game folder are written from the game folder, with forward slashes, as the game's mod places are: `r6/scripts`, `archive/pc/mod`.

## Voice

**We, not I.**
A committed document outlives the session that wrote it, and "I" leaves the reader guessing whether a person or an agent wrote it, and which one.
"We" is the project.
Admissions stay in the first person: "we haven't tested this on a Steam Deck" is someone talking, and "this has not been tested" reads as evasion.

**You, for the reader**, in procedures and agent instructions: "run the probe", "you'll see the copy update".

**Plain.**
No enthusiasm, no apology, no emoji, and no exclamation marks.
Contractions are fine wherever you'd say them aloud.

**American English**, with the Oxford comma.
Names keep their own spelling.

**The product is Choom Collector**, in prose, headings, commits, and anything a player reads.
A name that can't hold spaces takes one of these forms:

| Where | Form |
| --- | --- |
| Lowercase names: the repository, the program, and Velopack's pack ID | `choom-collector` |
| The library crates, named for their part | `coop-<part>`, as in `coop-logs` |
| Rust types, and folders on Windows | `ChoomCollector`, as in `%APPDATA%\ChoomCollector\` |
| Folders on Linux | `choom-collector`, as in `~/.config/choom-collector/` |

The game is Cyberpunk 2077, or the game.
The app it replaces is CP77 Log Collector.

## Words in the app

Everything a player reads in the window follows these rules, on top of the ones above.

- Players are named, never he or she, since the app doesn't know anyone's pronouns.
- Something happening now takes the progressive, "Fetching 4 of 12 files", "mira is reconnecting", and the present simple stays for how things work.
- A tooltip, toast, or status of one sentence takes no period.
  Two sentences take periods.
- Labels, buttons, and headings are in sentence case.
- One word for one thing, the same in every screen: session, host, tester, player, share, follow, mod, logs, relay, code.

## Words and patterns to cut

Each of these usually marks a sentence written on autopilot.
[prose-lint](../../tools/prose-lint.mjs) reads this table and flags every hit outside code, quotation marks, and URLs, including inflected forms (`robustness`, `leveraged`).
It checks Markdown, and the comments in JavaScript, TypeScript, Rust, C++, JSON, and TOML files, where it also flags history such as "previously" or "no longer", which [Code comments](#code-comments) rules out.
The fix is almost always to delete the word or say the concrete thing it stood in for.
A flagged word doing real work can stay: "actually" correcting a belief the text just stated is fine.

<!-- prose-lint: terms -->

| Cut                                                                                                                                                                                                                                               | Why                                                          | Instead                                                                          |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ | -------------------------------------------------------------------------------- |
| `genuinely`, `truly`, `really`, `actually`, `honestly`                                                                                                                                                                                            | Sincerity markers imply the rest is less sincere             | Delete                                                                           |
| `crucial`, `importantly`, `critically`, `vital`                                                                                                                                                                                                   | Asserts importance instead of showing it                     | State the consequence                                                            |
| `essentially`, `basically`, `fundamentally`                                                                                                                                                                                                       | Announces an approximation without saying what it leaves out | Say the exact thing                                                              |
| `simply`, `obviously`, `clearly`, `of course`, `straightforward`                                                                                                                                                                                  | Tells the reader how easy it should feel                     | Delete                                                                           |
| `robust`, `seamless`, `powerful`, `elegant`                                                                                                                                                                                                       | Praise that says nothing checkable                           | What it survives or does                                                         |
| `comprehensive`, `holistic`, `nuanced`                                                                                                                                                                                                            | Same                                                         | What it covers                                                                   |
| `leverage`, `utilize`, `utilise`                                                                                                                                                                                                                  | Longer words for "use"                                       | use                                                                              |
| `delve`, `streamline`, `empower`                                                                                                                                                                                                                  | Marketing verbs                                              | The literal verb                                                                 |
| `load-bearing`, `first-class`, `under the hood`, `game-changer`, `game changer`                                                                                                                                                                   | Stock metaphors                                              | What depends on it, or how it works                                              |
| `worth noting`, `note that`, `it's important to`, `it is important to`, `keep in mind`                                                                                                                                                            | Signposts that announce a sentence instead of writing it     | The sentence                                                                     |
| `let's`, `in summary`, `in conclusion`, `to summarize`, `as mentioned`, `as noted above`                                                                                                                                                          | Tour-guide narration                                         | Delete                                                                           |
| `as requested`, `as discussed`, `as you mentioned`, `you asked`                                                                                                                                                                                   | Leftovers from a conversation the reader wasn't in           | Delete                                                                           |
| `don't worry`, `rest assured`, `you might be wondering`, `you may be wondering`                                                                                                                                                                   | Reassurance nobody asked for                                 | Delete                                                                           |
| `in order to`                                                                                                                                                                                                                                     | Two wasted words                                             | to                                                                               |
| `a wide range of`, `a variety of`, `various`                                                                                                                                                                                                      | A quantity nobody counted                                    | Count them or name them                                                          |
| `currently`, `recently`, `at the moment`, `as of now`, `nowadays`                                                                                                                                                                                 | True only on the day it was written                          | A date, or nothing                                                               |
| `may potentially`, `could potentially`, `might potentially`, `could possibly`, `might possibly`                                                                                                                                                   | Stacked hedges                                               | One hedge, or a marker from [Evidence](#evidence)                                |
| `not just`, `not only`, `not merely`                                                                                                                                                                                                              | Argues with a claim nobody made                              | The positive claim                                                               |
| `successfully`                                                                                                                                                                                                                                    | Success is the default, so only failure needs saying         | Delete                                                                           |
| `colour`, `behaviour`, `favour`, `honour`, `neighbour`, `flavour`                                                                                                                                                                                 | British spelling                                             | color, behavior, favor, honor, neighbor, flavor                                  |
| `centre`, `centred`, `metre`, `litre`, `licence`, `defence`, `offence`                                                                                                                                                                            | British spelling                                             | center, meter, liter, license, defense, offense                                  |
| `grey`, `whilst`, `amongst`, `aeroplane`, `manoeuvre`, `tyre`, `aluminium`, `programme`, `analogue`, `catalogue`                                                                                                                                  | British spelling                                             | gray, while, among, airplane, maneuver, tire, aluminum, program, analog, catalog |
| `judgement`, `acknowledgement`, `learnt`, `spelt`                                                                                                                                                                                                 | British spelling                                             | judgment, acknowledgment, learned, spelled                                       |
| `/\b(organi\|recogni\|reali\|normali\|optimi\|initiali\|seriali\|synchroni\|minimi\|maximi\|prioriti\|summari\|customi\|authori\|categori\|finali\|visuali\|standardi\|characteri\|emphasi\|apologi)s(e\|es\|ed\|ing\|er\|ers\|ation\|ations)\b/` | British -ise spelling                                        | -ize: organize, synchronization                                                  |
| `/\b(travel\|cancel\|model\|label\|signal\|fuel\|level\|tunnel\|channel)l(ed\|ing\|er\|ers)\b/`                                                                                                                                                   | British doubled l                                            | traveled, modeling, labeled                                                      |

<!-- prose-lint: end -->

These patterns are beyond the lint, so the `doc-reviewer` agent looks for them:

- **The reflexive three:** three adjectives or clauses where one would do, chosen for rhythm.
- **The punchy restatement:** a paragraph that ends by saying itself again in a shorter sentence.
- **"It's not X, it's Y"** in any wording, when nobody claimed X.
- **Hedging a measurement:** if we measured it, it "does", not "appears to".
- **A missing Oxford comma.**
- **Unparallel lists:** list items, and headings at the same level, share a grammatical form.
  In agent instructions, a trigger heading ("Before a test run") can sit among noun headings, because the trigger is what gets it followed.

## Formatting

- Use prose for reasoning and lists for parallel items.
  A "because" chain broken into bullets loses its "because".
  Use a list when the items could be reordered without changing the meaning.
- Use a table to compare items on two or more attributes.
  A cell that needs a paragraph means the table is the wrong shape.
- Keep lists to two levels of nesting.
- Fence code blocks and give each a language tag, `text` for anything that isn't code.
  Put one command per block, with no prompt character and no output mixed in.
- Draw a diagram as text in a code block when it fits in 20 lines, and use Mermaid when it doesn't.
  A diagram shows a real mechanism with real names, not boxes labeled "Component A".
- Bold at most one phrase per paragraph: the one a skimmer must catch.
- Use at most one em dash per paragraph, with a space on each side.
  A colon, a period, or a pair of commas usually does better.
- Put a callout box (`> **Note**`, GitHub alerts) only on an amendment pointer or on a warning that costs something if ignored.
- Write paths with placeholders, never a real home folder: `%APPDATA%\ChoomCollector\`, `<user>`.
- Quote sources briefly, and link to them or cite them as [Evidence](#evidence) says.

## Code comments

A comment's reader is working on the code and hasn't read `record/`.
The code already states what it does, so a comment earns its place by saying what the code can't: why it's this way, the input that breaks it, the ordering that matters, or the thing that was tried and didn't work.

1. **When a comment explains why the code is this way, open with the stakes:** what the code makes possible or prevents, then the fact that shapes it, then the choice the code makes because of it.
   A fact that doesn't lead to the code below gets cut, because a reader forgets facts that connect to nothing.
2. **Define a project term in a clause before using it**, as in "the host, the PC every tester's logs go to", and explain an oddity before the rule that handles it.
3. **Cite only what a reader would doubt and couldn't check quickly**, with at most one pointer, at the end.
   A workaround for the game's, Windows', or Proton's behavior always cites how we know, as a document does: a log entry, a document in `record/`, or a vendor page.
   Skip the citation for anything a compiler or tool reports the moment it breaks.
4. **Where a value lives in two places, say so in both:** "`VPK_VERSION` in release.yml names the same version, so change both together."
5. **Show a format by example, and make the example explain itself:** "`r6/scripts/EquipmentEx` is one of the mod's folders", not "a path such as `r6/scripts/X`".
6. **Make each sentence follow from the one before.**
7. **Start a sentence that tells the reader what to do on a later change with its trigger:** "When you add a mod place, add it to `MOD_PLACES` and to the scope's list."
8. **Use the present tense for what holds:** "Windows refuses the rename while the game holds the file", not "refused".
   If the reader needs the measurement behind it, give it a past-tense sentence of its own with its citation, and otherwise leave it in the record.

```text
✗  // Retry the replace later.
✓  // While the game runs it holds every archive it loaded, and Windows refuses to
   // replace a held file, so a failed replace waits for the game to let go instead of
   // failing the follow. (read in CP77 Log Collector, README.md, "Sharing mods")
```

A Rust `expect` message says what should have held, as in "the settings folder should be writable", since the panic prints it before the error.
A TODO names the condition that ends it.
Leave out the code's own history, such as "added for the relay change", because git keeps it.
Leave out plan and status, such as a stage number, because they go stale.
Leave out commented-out code.

## Commit messages

Commits follow [Conventional Commits](https://www.conventionalcommits.org/), because release-please reads them to choose the next version and to write the changelog.
The title is the changelog line, so it says what changed for a player, in the voice above: `feat: show which logs each player's PC picks up`.

- `feat:` is something a player notices, and `fix:` is a bug a player could hit.
- `docs:` is the record, `test:` is tests, `ci:` and `build:` are the pipeline and the build, `refactor:` is a change no player notices, and `chore:` is the rest.
- A breaking change to the protocol, which needs everyone to update together, gets `!` after the type and says so in the body.

## Instructions for agents

`CLAUDE.md`, skills, agent definitions, and prompts are read by a model that follows them literally and can't ask what they meant.

- **Every line changes behavior.**
  Delete a line and ask whether an agent would act differently.
  If not, leave it deleted.
  A new rule's reason sentence names the failure that prompted it.
- **Rule first, then the reason in one sentence.**
  The reason is what lets an agent handle the case the rule didn't name.
- **Name the trigger.**
  "Before a test run" gets followed, and a free-floating rule is forgotten by the time it applies.
- **No shouting.**
  Capitals, "IMPORTANT", and "you must always" make a model apply a rule to cases it was never meant for.
  A calm rule with a reason is followed more accurately.
- **One example beats three adjectives.**
  A before-and-after pair teaches what "concise" can't.
- **Point, don't paste.**
  Link to the canonical document.
  A rule that must be seen before an action lives inline in `CLAUDE.md`, and then only there.
- **Keep `CLAUDE.md` under 150 lines of its own.**
  It's read at the start of every session, whether or not it's relevant.
  Every file in `.claude/rules/` without `paths:` in its front matter loads with it, so each of their lines faces the same deletion test.
- **An agent definition that writes tells its agent to read this file**, rather than relying on Claude Code loading it for the agent.

## Finishing a document

1. Read it once, at speed, as its reader, and note each place you slowed down.
2. Rewrite each of those places.
3. If you wrote or substantially changed a document in `record/`, `CLAUDE.md`, or a Markdown file in `.claude/`, run the `doc-reviewer` agent.
   Fix or answer each finding.
   You're done when its report opens by saying the document is ready.

If the document is a log written during an experiment, skip step 3.
Run it when a finding moves into a design doc or a decision.
