brace_count = 0
line_num = 0

with open('src/main.rs', 'r') as file:
    for line in file:
        line_num += 1
        for c in line:
            if c == '{':
                brace_count += 1
            elif c == '}':
                brace_count -= 1
                if brace_count < 0:
                    print(f"Too many closing braces at line {line_num}")
                    exit(1)
                
print(f"Missing {brace_count} closing braces")
