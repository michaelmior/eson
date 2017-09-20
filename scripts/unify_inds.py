import collections
import sys

inds = collections.defaultdict(lambda: [])
other_lines = []

for line in open(sys.argv[1]).readlines():
    if '<=' not in line:
        other_lines.append(line)
        continue

    line = line.replace(', ', ',').replace('(', ' ').replace(')', '').strip()
    left_table, left_fields, _, right_table, right_fields = line.split(' ')
    left_fields = left_fields.split(',')
    right_fields = right_fields.split(',')

    inds[(left_table, right_table)].append((left_fields, right_fields))

out = open(sys.argv[1], 'w')

# Write all lines before the INDs
blanks = 0
while True:
    line = other_lines.pop(0)
    out.write(line)
    if line.strip() == '':
        blanks += 1
    if blanks > 1:
        break

# Print the collected INDs
for (left_table, right_table), ind_list in inds.items():
    for (left_fields, right_fields) in ind_list:
        if (right_fields, left_fields) in  inds[(right_table, left_table)]:
            if left_table < right_table:
                continue
            out.write('%s %s == %s %s\n' % (left_table, ', '.join(left_fields),
                                            right_table, ', '.join(right_fields)))
        else:
            out.write('%s %s <= %s %s\n' % (left_table, ', '.join(left_fields),
                                            right_table, ', '.join(right_fields)))

# Write all lines after the INDs
out.writelines(other_lines)
