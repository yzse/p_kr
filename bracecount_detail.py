brace_count = 0
last_open_lines = []

with open('src/main.rs', 'r') as file:
    for line_num, line in enumerate(file, 1):
        for char_idx, char in enumerate(line):
            if char == '{':
                brace_count += 1
                last_open_lines.append((line_num, line.strip(), brace_count))
            elif char == '}':
                brace_count -= 1
                if last_open_lines:
                    last_open_lines.pop()

print(f"Brace count at end: {brace_count}")
if brace_count > 0:
    print("Last few opening braces without matching closers:")
    for line_num, line_text, count in last_open_lines[-5:]:
        print(f"Line {line_num}: {line_text} (count: {count})")
