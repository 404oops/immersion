const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

// Exercise the actual trusted workflow script against a simulated GitHub API.
const workflow = fs.readFileSync(path.join(__dirname, '../workflows/agent-disclosure.yml'), 'utf8');
const source = workflow.split('          script: |\n')[1]
  .split('\n').map(line => line.replace(/^ {12}/, '')).join('\n');
const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
const run = new AsyncFunction('context', 'github', 'core', source);
const footer = '(This PR was made with an agent in partial or full capacity)';

async function check({ body = '', branch = 'fix/storage', messages = [], labelStatus, createStatus } = {}) {
  const failures = [], labels = [], calls = [];
  let labelExists = labelStatus === undefined;
  const github = {
    rest: {
      pulls: { listCommits: Symbol('listCommits') },
      issues: {
        async getLabel() {
          calls.push('getLabel');
          if (!labelExists) throw Object.assign(new Error('label lookup'), { status: labelStatus });
        },
        async createLabel() {
          calls.push('createLabel');
          if (createStatus) {
            if (createStatus === 422) labelExists = true; // Concurrent creation.
            throw Object.assign(new Error('label creation'), { status: createStatus });
          }
          labelExists = true;
        },
        async addLabels(args) { labels.push(...args.labels); }
      }
    },
    async paginate(method, args) {
      assert.equal(method, github.rest.pulls.listCommits);
      assert.equal(args.per_page, 100);
      return messages.map(message => ({ commit: { message } }));
    }
  };
  await run({ repo: { owner: '404oops', repo: 'immersion' }, payload: {
    pull_request: { number: 12, body, head: { ref: branch } }
  } }, github, { setFailed: message => failures.push(message) });
  return { failures, labels, calls };
}

test('ordinary human PR passes, including null body and human coauthor', async () => {
  assert.deepEqual((await check({ body: null, messages: ['Fix\n\nCo-Authored-By: Alice <alice@example.com>'] })).failures, []);
});

test('agent commit trailers and AI coauthors require the footer', async () => {
  for (const message of ['Fix\n\nAgent-Assisted: yes', 'Fix\n\nCo-Authored-By: Claude <noreply@example.com>', 'Fix\n\nCo-Authored-By: AI <ai@example.com>', 'Fix\n\nCo-Authored-By: GitHub Copilot <copilot@example.com>']) {
    assert.equal((await check({ messages: [message] })).failures.length, 1);
    assert.equal((await check({ messages: [message], body: footer })).failures.length, 0);
  }
});

test('agent branch boundaries, declarations, and late commits are detected', async () => {
  for (const branch of ['codex/fix', 'fix/agent-storage', 'ai/fix', 'feature/copilot_fix']) {
    assert.equal((await check({ branch })).failures.length, 1);
  }
  assert.equal((await check({ branch: 'fix/agency' })).failures.length, 0);
  assert.equal((await check({ body: '- [X] I used an AI agent: yes' })).failures.length, 1);
  assert.equal((await check({ messages: [...Array(110).fill('Human fix'), 'Agent-Assisted: yes'] })).failures.length, 1);
});

test('footer must be the exact final line', async () => {
  for (const body of [`${footer}\nMore text`, `Prefix ${footer}`, footer.replace('agent', 'AI'), ` ${footer}`]) {
    assert.equal((await check({ branch: 'codex/fix', body })).failures.length, 1);
  }
  assert.equal((await check({ branch: 'codex/fix', body: `Details\r\n${footer}\r\n` })).failures.length, 0);
});

test('marker labels PRs whether or not footer exists', async () => {
  for (const body of ['biblioklept', `biblioklept\n${footer}`]) {
    const result = await check({ body, labelStatus: 404 });
    assert.deepEqual(result.labels, ['disclosure-violation']);
    assert.ok(result.calls.includes('createLabel'));
    assert.equal(result.failures.length, body.includes(footer) ? 0 : 1);
  }
  assert.deepEqual((await check({ body: footer })).labels, []);
});

test('concurrent label creation is tolerated but API errors fail', async () => {
  assert.deepEqual((await check({ body: `biblioklept\n${footer}`, labelStatus: 404, createStatus: 422 })).labels, ['disclosure-violation']);
  await assert.rejects(check({ body: 'biblioklept', labelStatus: 403 }));
  await assert.rejects(check({ body: 'biblioklept', labelStatus: 404, createStatus: 500 }));
});

test('uninspectable commit counts fail closed', async () => {
  assert.equal((await check({ body: footer, messages: Array(250).fill('Fix') })).failures.length, 1);
});
