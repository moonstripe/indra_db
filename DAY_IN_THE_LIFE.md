# Day in the Life: Marcus, Solo Developer with Claude Code

## The Persona

**Marcus Chen** is a 28-year-old freelance web developer in Portland, OR. He uses Claude Code as his daily pair programmer through his terminal. He works on 3-4 client projects simultaneously, building full-stack applications with React and Node.js.

## The Pain

It's Tuesday morning. Marcus opens his terminal to continue building an e-commerce checkout flow for a client. He starts a new Claude Code session:

> "Hey, let's continue on the checkout flow. Can you remind me what we decided about the payment provider?"

Claude doesn't know. It's a new session — the context is gone.

Marcus sighs and types out the full context again: "We're using Stripe, but the client wants to support Apple Pay. Last session we decided to use Stripe's Payment Element instead of individual components because..."

He does this **every single morning**. Re-explaining his tech stack, his coding conventions, his client's preferences, the architectural decisions they've already made. It's like working with a colleague who has amnesia.

Worse, last week Claude recommended using `express-validator` for input validation. Two days later, in a new session, it recommended `zod` — contradicting its own earlier advice without realizing it. Marcus caught it, but what about the times he didn't?

## The Discovery

Marcus sees a tweet: *"I gave my AI assistant a memory that persists across sessions. It remembers my preferences, my past decisions, and why I made them."*

The linked repo is `indra_db` — a local-first database that works as an MCP server for Claude Code. He installs it in 2 minutes:

```bash
cargo install indra_db
# Add to Claude Code's MCP config
```

## The After

It's Wednesday morning. Marcus opens his terminal:

> "Let's continue on the checkout flow."

Claude searches its Indra memory, finds notes about the project, and responds:

> "Welcome back! Based on our previous sessions, we're building the checkout for the Greenleaf client using Stripe Payment Element (we chose this over individual components for Apple Pay support). Yesterday we finished the CartSummary component and were about to start on the shipping address form. You prefer using zod for validation with react-hook-form. Want to pick up there?"

No re-explanation needed.

When Marcus asks about validation libraries, Claude checks its memory and says: *"We've been using zod throughout this project — I recommended it on Feb 3rd because it pairs well with your TypeScript-first approach."* Consistency.

And when the client changes the spec mid-project, Marcus can see exactly how the project's understanding evolved:

```bash
indra log
# Feb 8: "Client wants gift cards — adding coupon/gift-card field to checkout"
# Feb 6: "Shipping form done. Client confirmed US-only for now."
# Feb 5: "Using Stripe Payment Element for Apple Pay support"
# Feb 3: "New project: Greenleaf e-commerce. React + Node, zod, Tailwind"
```

## The Outcome

Marcus saves **15-20 minutes per session** on context re-establishment. Over a month, that's 5+ hours of productivity gained. More importantly, his AI assistant makes *consistent* recommendations and builds on previous decisions instead of contradicting them.

He tells his developer friends: *"It's like the difference between working with a contractor who keeps notes and one who shows up fresh every day."*

---

**Primary ICP: Solo developers using AI coding assistants who work on multiple projects and lose context between sessions.**

Key value drivers:
1. **Session persistence** — Agent remembers across conversations
2. **Decision consistency** — No more contradictory recommendations  
3. **Context evolution** — See how understanding changed over time
4. **Zero friction** — Local-first, installs in 2 minutes, works with existing tools
