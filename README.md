# LivingDocs

> Documentation that keeps up with your code. Unlike Dave.

A command-line tool you install once, schedule once, and then never have to think about — while your docs quietly stop lying.

---

## The Problem

Software documentation follows a predictable lifecycle:

### Day 1

```text
README.md
├── Accurate
├── Helpful
└── Recently Updated
```

### Day 30

```text
README.md
├── Mostly Accurate
├── Slightly Suspicious
└── Missing Several Things
```

### Day 180

```text
README.md
├── Historical Fiction
├── References a Service That No Longer Exists
└── Mentions "Upcoming Features" Already Removed Last Quarter
```

At some point, the documentation becomes less of a guide and more of an archaeological artifact.

---

## Meet LivingDocs

LivingDocs is a CLI that evolves your documentation alongside your project.

It is **not** a chatbot bolted onto your repo. You already have five of those.

It is the thing none of those do: it reads your actual code, notices when your docs have started lying, fixes the parts it generated, and opens a pull request — on whatever schedule you set. You install it, you schedule it, and then it does its job while you do yours.

No more opening a repository and discovering:

* An architecture diagram from three reorganizations ago
* Setup instructions that have never worked
* A "Getting Started" guide that starts with "Ask Rahul"
* A README proudly describing features removed in 2024

LivingDocs helps keep documentation alive, relevant, and occasionally smarter than the people maintaining it.

---

## Install

```bash
npm install -g livingdocs
export OPENAI_API_KEY=sk-...
```

That's the whole setup. It's a CLI. It does not want to live in your editor.

---

## Quick Start

```bash
cd my-project

livingdocs init        # scaffold config + managed doc sections
livingdocs analyze     # read the code, build the graph, write the first docs
livingdocs check       # tell me which docs are now lying (exits non-zero if any)
livingdocs update      # fix the lies, open a PR, leave the rest of my prose alone
```

`check` is the one you put in CI. `update` is the one you schedule.

---

## What It Writes Into Your Repo

```text
docs/
├── index.md             # table of contents, kept current
├── overview.md          # what this repo even is
├── architecture.md      # the diagram that's finally correct
├── components/          # one file per service/module
├── apis/                # your routes, documented
├── diagrams/            # Mermaid that redraws itself
├── dependencies.md      # what you depend on, like it or not
└── data-model.md        # your entities, when we can find them

.livingdocs/             # the tool's notes-to-self (graph + drift state)
```

The generated bits live between `<!-- LIVINGDOCS:BEGIN -->` markers, each tagged with the code it describes so it knows exactly what went stale. Everything you write by hand, outside those markers, stays untouched. It colors inside its own lines.

---

## Set It and Forget It

The entire point. Schedule LivingDocs and it keeps your docs honest with no human in the seat.

Drop this in `.github/workflows/livingdocs.yml`:

```yaml
on:
  schedule:
    - cron: "0 6 * * 1"   # every Monday at 6am, before anyone is awake to argue
jobs:
  livingdocs:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: npm install -g livingdocs
      - run: livingdocs update --pr
        env:
          OPENAI_API_KEY: ${{ secrets.OPENAI_API_KEY }}
```

Every Monday it wakes up, checks what drifted, fixes what it generated, and opens a pull request titled something like *"4 docs drifted, here are the fixes."* You review it with your coffee. You merge it or you don't. Nobody scheduled a meeting.

Prefer a different cadence? It's a cron string. Prefer a git hook? Run `livingdocs check` on pre-push and block drift before it ships. Prefer to never automate anything and run it by hand at 2am in a panic before a demo? That also works, and we won't judge. (We will judge a little.)

---

## What Can It Do?

### Catch Documentation That Started Lying

This is the whole point.

LivingDocs reads your actual code, then reads your docs, and notices the disagreements:

```text
docs/architecture.md:42  drift  references "Redis", no Redis dependency since March
```

It exits non-zero, so your CI can treat a lying doc like a failing test.

---

### Keep Documentation Fresh By Itself

Anything LivingDocs generates, it keeps current.

Diagrams, component summaries, API tables — all live between tidy little markers:

```md
<!-- LIVINGDOCS:BEGIN -->
(the part that updates itself)
<!-- LIVINGDOCS:END -->
```

Everything you wrote by hand stays exactly where you left it. Everything it wrote stays true.

Documentation that notices reality has changed. Revolutionary, we know.

---

### Draw Diagrams That Don't Rot

Because every architecture eventually becomes:

```text
┌─────────┐
│ Stuff   │
└────┬────┘
     │
     ▼
┌─────────┐
│ More    │
│ Stuff   │
└─────────┘
```

LivingDocs turns "stuff" into actual Mermaid diagrams — and redraws them when the code moves, so they never quietly become wrong.

---

### Also, Yes, It Can Explain Things

```bash
livingdocs explain UserService
```

Instead of:

```text
Step 1: Ask somebody.
Step 2: Ask somebody else.
Step 3: Realize the expert left two years ago.
```

Get a useful answer, grounded in your code.

(We list this last on purpose. It's handy. It is not the reason LivingDocs exists. For deep questions, your favorite cloud assistant is still smarter — LivingDocs is here to keep the *written record* honest.)

---

### Help New Team Members

Instead of onboarding looking like:

```text
New Developer:
"Where should I start?"

Team:
"Good question."
```

LivingDocs gives newcomers a fighting chance — with docs they can actually trust, because the ones that drifted got flagged.

---

## Why We Built It

Because every engineering team eventually reaches the same conclusion:

> "We should really update the documentation."

And then immediately schedules a meeting to discuss updating the documentation.

LivingDocs skips directly to the part where the documentation gets updated — and, better yet, schedules *itself* to do it.

---

## Who Is It For?

* Developers inheriting mysterious codebases
* Architects drawing their 47th diagram this quarter
* Team leads answering the same question for the sixth time this week
* Anyone who has ever said:

  > "That's not how the system works anymore."

---

## Frequently Asked Questions

### Is this a VS Code extension?

No. It's a CLI. It lives in your terminal and your CI, not your editor. (An editor client may come later. The CLI is the product.)

### Isn't this just another "chat with your repo" tool?

No. You can ask it things, but that's the garnish.

The main course is drift detection on a schedule: it tells you — and fixes — which docs have quietly become false, every week, without being asked. No chatbot does that, because a chat answer disappears the second you close the tab. Docs don't.

### Is LivingDocs AI-powered?

Yes — but the part that decides whether a doc is *wrong* is deterministic, not vibes. OpenAI writes the prose; your local code graph decides the facts.

### Does my code leave my machine?

For the writing step, yes. LivingDocs sends a structured summary of your code — entities, dependencies, routes — to OpenAI to generate the prose and diagrams. It sends the *graph*, not your raw files.

The part that decides whether a doc is *wrong* never leaves your machine: drift detection is local graph math, no API call. You'll need an `OPENAI_API_KEY`.

### Will it push weird AI commits straight to main while I sleep?

No. It opens a pull request. A human merges it. Your `main` branch is safe from its enthusiasm.

### Will it replace engineers?

No.

### Will it replace documentation?

No. It replaces *out-of-date* documentation, which is arguably worse than none.

### Can it explain a 10-year-old enterprise application?

It can try.

We believe everyone deserves emotional support when opening a legacy codebase.

### Does it judge my code?

Officially, no.

Unofficially, we have no comment.

---

## Roadmap

### Today

A CLI that flags its own lies, keeps the generated parts honest, and runs itself on a schedule.

### Tomorrow

That, plus understanding your git history and commenting drift directly on pull requests.

### The Future

Documentation that gently reminds people, on the pull request itself:

> "You changed six services and updated none of the docs."

---

## Contributing

Contributions are welcome.

Especially documentation contributions.

The irony is not lost on us.

---

## License

MIT

Because documentation should be easier to share than tribal knowledge.

---

**LivingDocs**

*Making documentation slightly less fictional, one scheduled run at a time.*
