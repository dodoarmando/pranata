export default {
  extends: ['@commitlint/config-conventional'],
  rules: {
    'scope-enum': [
      2,
      'always',
      ['api', 'web', 'db', 'infra', 'ci', 'deps', 'release', 'adr', 'context', 'docs'],
    ],
    // DCO sign-off (ADR-0002).
    'trailer-exists': [2, 'always', 'Signed-off-by:'],
  },
};
