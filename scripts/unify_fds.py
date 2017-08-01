import collections
import sys

fds = collections.defaultdict(lambda: collections.defaultdict(lambda: []))
other_lines = []

for line in open(sys.argv[1]).readlines():
    # Collect lines which are not FDs
    if '->' not in line:
        other_lines.append(line)
        continue

    # Collect all FDs by the LHS
    line = line.replace(', ', ',').strip()
    table, lhs, _, rhs = line.split(' ')
    lhs = tuple(lhs.split(','))
    rhs = rhs.split(',')
    fds[table][lhs].extend(rhs)

out = open(sys.argv[1], 'w')

# Write all liens before the INDs
while True:
    line = other_lines.pop(0)
    out.write(line)
    if line.strip() == '':
        break

# Write each collected FD
for table, fds in fds.items():
    for lhs, right_fields in fds.items():
        out.write('%s %s -> %s\n' % (table, ', '.join(lhs), ', '.join(right_fields)))

# Write all lines after the FDs
out.writelines(other_lines)
