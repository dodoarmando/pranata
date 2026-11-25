/**
 * Hook options for a mutation whose result holds a secret, such as an Access
 * Link URL. By default TanStack Query keeps a finished mutation, result
 * included, in its cache for five minutes after the last component stops
 * observing it. With `gcTime: 0` it is dropped as soon as the component calls
 * `reset()` or unmounts.
 */
export const SECRET_RESULT = { mutation: { gcTime: 0 } } as const;
