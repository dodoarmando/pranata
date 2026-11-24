# Interactive UI only through shadcn components, enforced by lint

shadcn components (Base UI variant) live in `apps/web/src/components/ui`. Feature code may not render raw `<button>`, `<input>`, `<select>`, `<textarea>` or `<dialog>`; a Biome GritQL rule fails CI when it does.

In the archived attempt shadcn was installed and agreed on, yet pages were built without it until a corrective issue. A rule that fails the build keeps the decision alive without anyone remembering it.

## Considered Options

- **Convention and review only**: what failed before.
- **A custom component library**: more control, but more to build and maintain than adopting shadcn's copied-in components.

## Consequences

Components in `components/ui` are exempt from the rule. A new kind of control first gets a `ui/` component, then feature code uses it.
