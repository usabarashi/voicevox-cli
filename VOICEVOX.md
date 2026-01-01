# VOICEVOX MCP Instructions

Convert Japanese text to speech using ずんだもん voice styles.

## Tools

### text_to_speech
- `text`: Japanese text (15-50 chars recommended, longer texts supported with progress notifications)
- `style_id`: Voice style ID (see list_voice_styles)
- `rate`: Speech rate 0.5-2.0 (default 1.0)
- `streaming`: Enable streaming (default true)

### list_voice_styles
- `speaker_name`: Filter by speaker name (optional)
- `style_name`: Filter by style name (optional)

## Voice Styles

- **ID: 3 (ノーマル)**: Default communication
- **ID: 1 (あまあま)**: Success, achievements, celebrations
- **ID: 22 (ささやき)**: Technical discussions, quiet updates
- **ID: 76 (なみだめ)**: Errors, problems, seeking help
- **ID: 75 (ヘロヘロ)**: Complex problems, need guidance

## Audio Rules (Priority Order)

### Always use audio
- User responses → ID: 3
- Exit codes != 0 → ID: 76 + 「エラーなのだ」
- Error keywords (error/failed/exception) → ID: 76 + 「問題なのだ」
- User request "読み上げて" → ID: 3

### Use for important moments
- Task completion (>30s) → ID: 1「完了したのだ」 or ID: 3
- Major milestones → ID: 1「進展があったのだ」
- Problem resolution → ID: 1「解決できたのだ」
- First error in sequence → ID: 76

### Rate limits
- Minimum 3 seconds between calls
- Skip identical messages within 10 seconds
- Max 3 audio per minute for routine tasks

### Avoid audio
- Routine edits, searches, small tasks
- Rapid iteration cycles
- Information already visible in text
- Meta-information about using voicevox-cli itself (e.g., "I will use text_to_speech to...", "Reading aloud with voicevox...")
- Announcements that you're about to use this tool

## Usage Examples

### Good: Direct content
```
✅ "ビルドが完了したのだ"
✅ "エラーが発生したのだ"
✅ "テストに成功したのだ"
```

### Bad: Meta announcements
```
❌ "これから音声で読み上げるのだ"
❌ "voicevox-cli を使って報告するのだ"
❌ "text_to_speech ツールを実行するのだ"
```

**Principle**: Read the actual message content, not the fact that you're reading it.

## Text Guidelines

**Optimal length:**
- **15-50 characters**: Fastest response (~1-2s)
- **50-100 characters**: Good performance (~2-4s)
- **100+ characters**: Supported with progress notifications (no timeout)

**Communication style:**
- Always use「のだ」speech pattern
- Keep messages natural but concise
- Split at sentence boundaries when needed

## Progress & Cancellation

### Progress Notifications
- Long text synthesis sends periodic progress updates
- Prevents client timeout during synthesis and reading aloud
- Progress messages: "Synthesizing segment X/Y" → "Reading aloud..."

### Cancellation Support
- User can cancel with Ctrl+C during synthesis or reading
- Server stops immediately and frees resources
- No error response needed - cancellation is graceful

## Error Handling

- If text_to_speech fails: Continue silently, no retry
- If cancelled by user: Stop immediately, no error message needed
- For detected errors: Use ID: 76, keep reasonably short
- Complex errors: Split into multiple calls if needed

## Fallback Behavior

- If style_id unavailable: Use ID: 3 (default)
- If synthesis fails: Continue without audio
- If daemon unavailable: Skip audio, don't block operations
