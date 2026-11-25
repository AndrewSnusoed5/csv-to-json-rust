import {readFileSync, writeFileSync} from 'node:fs';
import {parse} from 'csv-parse/sync';
const input = readFileSync(process.argv[2], 'utf8');
const records = parse(input, {
  delimiter: process.argv[3],
  record_delimiter: process.argv[4] === 'CRLF' ? '\r\n' : '\n',
  columns: process.argv[5] === 'header',
  skip_empty_lines: process.argv[6] === 'skip',
  bom: true,
});
writeFileSync('rows.json', JSON.stringify(records), 'utf8');
console.log(records.length);
