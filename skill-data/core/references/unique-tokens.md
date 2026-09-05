# Unique fields and template tokens

Use `fill-unique` for a value that the target rejects when reused. It fills a
record-time value and writes a replay-native `do` draft with the original
template. Replay mints a new value for each run.

```bash
agent-qa fill-unique "Account name" --template '{{vars._unique}}'
agent-qa fill-unique "Email" --template 'qa-{{vars._unique}}@example.com'
```

Use a literal `record-step do` draft when a field must match an existing value.

```bash
agent-qa record-step do '{
  "intent": "enter an existing email",
  "verb": "type",
  "on": { "role": "textbox", "name": "Email" },
  "value": { "from": "literal", "literal": "person@example.com" }
}'
```

`--save-as` still records a local binding for compatibility. It does not add a
new scenario language. Keep replay preconditions in `record-setup`.
