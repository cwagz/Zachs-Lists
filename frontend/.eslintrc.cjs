module.exports = {
  root: true,
  env: { browser: true, es2022: true },
  extends: ['eslint:recommended'],
  ignorePatterns: ['dist'],
  overrides: [
    {
      files: ['*.ts', '*.tsx'],
      parser: '@typescript-eslint/parser',
      parserOptions: {
        ecmaVersion: 'latest',
        sourceType: 'module',
        ecmaFeatures: { jsx: true },
      },
      plugins: ['react-hooks'],
      rules: {
        'no-undef': 'off',
        'no-unused-vars': 'off',
        'react-hooks/rules-of-hooks': 'error',
      },
    },
    {
      files: ['*.config.js', '*.config.ts', '*.cjs'],
      env: { node: true },
    },
  ],
};
