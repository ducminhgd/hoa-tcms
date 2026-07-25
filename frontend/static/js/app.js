// HOA TCMS — Main application with page handlers
(function() {
'use strict';

// === Helpers ===

function h(tag, attrs, ...children) {
  const el = document.createElement(tag);
  if (attrs) Object.entries(attrs).forEach(([k, v]) => {
    if (k.startsWith('on')) el.addEventListener(k.slice(2).toLowerCase(), v);
    else if (k === 'class') el.className = v;
    else if (k === 'innerHTML') el.innerHTML = v;
    else if (k === 'style') Object.assign(el.style, v);
    else el.setAttribute(k, v);
  });
  children.forEach(c => {
    if (typeof c === 'string') el.appendChild(document.createTextNode(c));
    else if (c instanceof Node) el.appendChild(c);
  });
  return el;
}

function renderPage(container, title, contentFn) {
  const div = document.createElement('div');
  div.className = 'page';
  div.innerHTML = `<div class="page-header"><h1>${title}</h1></div>`;
  const body = document.createElement('div');
  if (contentFn) contentFn(body);
  div.appendChild(body);
  container.innerHTML = '';
  container.appendChild(div);
}

function toast(msg, type = 'info') {
  const container = document.getElementById('toast-container');
  const t = document.createElement('div');
  t.className = `toast ${type}`; t.textContent = msg;
  container.appendChild(t);
  setTimeout(() => t.remove(), 3500);
}

function pagination(page, total, limit, cb) {
  const totalPages = Math.max(1, Math.ceil(total / limit));
  return h('div', { class: 'pagination' },
    h('span', { class: 'pagination-info' }, `Page ${page} of ${totalPages} (${total} total)`),
    h('button', { class: 'btn btn-sm', disabled: page <= 1 ? 'true' : null, onclick: () => cb(page - 1) }, '← Previous'),
    h('button', { class: 'btn btn-sm', disabled: page >= totalPages ? 'true' : null, onclick: () => cb(page + 1) }, 'Next →')
  );
}

function dataTable(columns, rows, actionsFn) {
  const table = h('table', { class: 'data-table' });
  const thead = h('thead');
  thead.appendChild(h('tr', {}, h('th', {}, h('input', { type: 'checkbox' })),
    ...columns.map(c => h('th', {}, c)),
    h('th', {}, 'Actions')));
  table.appendChild(thead);
  const tbody = h('tbody');
  rows.forEach((row, i) => {
    const tr = h('tr');
    tr.appendChild(h('td', {}, h('input', { type: 'checkbox', value: row[0] })));
    row.forEach((cell, j) => {
      if (j === 0) return; // skip id (already in checkbox)
      const td = h('td');
      if (j === 1) {
        td.innerHTML = cell; // allow HTML in first column (links)
      } else {
        td.textContent = cell || '';
      }
      tr.appendChild(td);
    });
    const tdActions = h('td', { class: 'actions' });
    if (actionsFn) actionsFn(tdActions, row, i);
    tr.appendChild(tdActions);
    tbody.appendChild(tr);
  });
  table.appendChild(tbody);
  return table;
}

function statusBadge(status) {
  const s = (status || '').toLowerCase().replace('_', '-');
  return `<span class="badge badge-${s}">${status}</span>`;
}

// === Login ===
function loginPage(container) {
  container.innerHTML = '';
  container.style.marginLeft = '0';
  const div = document.createElement('div');
  div.className = 'login-page';
  div.innerHTML = `
    <form class="login-form" id="login-form">
      <h1>HOA TCMS</h1>
      <div class="form-group">
        <label>Username or Email</label>
        <input type="text" id="login-user" required autofocus />
      </div>
      <div class="form-group">
        <label>Password</label>
        <input type="password" id="login-pass" required />
      </div>
      <button type="submit" class="btn btn-primary" style="width:100%">Login</button>
      <p class="error-msg" id="login-error"></p>
    </form>`;
  container.appendChild(div);
  document.getElementById('login-form').addEventListener('submit', async (e) => {
    e.preventDefault();
    const user = document.getElementById('login-user').value;
    const pass = document.getElementById('login-pass').value;
    const errEl = document.getElementById('login-error');
    try {
      await api.login({ username_or_email: user, password: pass });
      router.navigate('/projects');
      location.reload();
    } catch (err) {
      errEl.textContent = err.message;
      errEl.classList.add('show');
    }
  });
}

// === Projects ===
function projectList(container) {
  renderPage(container, 'Projects', (body) => {
    const addBtn = h('button', { class: 'btn btn-primary', onclick: () => router.navigate('/projects/new') }, '+ Add New');
    body.parentElement.querySelector('.page-header').appendChild(addBtn);

    const tableDiv = h('div');
    const pagDiv = h('div');
    body.appendChild(tableDiv);
    body.appendChild(pagDiv);

    function load(page = 1) {
      api.listProjects(page).then(resp => {
        const cols = ['ID', 'Name', 'Status', 'Created'];
        const rows = resp.data.map(p => [p.id, `<a href="#/projects/${p.id}">${p.name}</a>`, statusBadge(p.status), p.created_at?.split('T')[0] || '']);
        tableDiv.innerHTML = '';
        tableDiv.appendChild(dataTable(cols, rows, (td, row) => {
          td.appendChild(h('button', { class: 'btn-icon', title: 'Edit', onclick: () => router.navigate(`/projects/${row[0]}/edit`) }, '✏️'));
          td.appendChild(h('button', { class: 'btn-icon', title: 'Delete', onclick: () => { if (confirm('Delete?')) api.deleteProject(row[0]).then(() => load(page)).catch(e => toast(e.message, 'error')); } }, '🗑️'));
        }));
        pagDiv.innerHTML = '';
        pagDiv.appendChild(pagination(page, resp.meta.total, resp.meta.limit, load));
      }).catch(e => toast(e.message, 'error'));
    }
    load();
  });
}

function projectForm(container, params) {
  const isEdit = !!params.id;
  renderPage(container, isEdit ? 'Edit Project' : 'Create Project', (body) => {
    let project = null;
    const form = h('form', { class: 'form', onsubmit: async (e) => {
      e.preventDefault();
      const data = { name: form.name.value, description: form.description.value || null, status: form.status.value };
      try {
        if (isEdit) await api.updateProject(params.id, data);
        else await api.createProject(data);
        toast(isEdit ? 'Updated' : 'Created', 'success');
        router.navigate('/projects');
      } catch (err) { toast(err.message, 'error'); }
    }});
    form.innerHTML = `
      <div class="form-group"><label>Name</label><input name="name" required /></div>
      <div class="form-group"><label>Description</label><textarea name="description"></textarea></div>
      <div class="form-group"><label>Status</label><select name="status"><option value="ACTIVE">Active</option><option value="INACTIVE">Inactive</option></select></div>
      <div class="form-actions">
        <button type="submit" class="btn btn-primary">${isEdit ? 'Update' : 'Create'}</button>
        <button type="button" class="btn" onclick="router.navigate('/projects')">Cancel</button>
      </div>`;
    body.appendChild(form);

    if (isEdit) {
      api.getProject(params.id).then(resp => {
        const p = resp.data;
        form.name.value = p.name || '';
        form.description.value = p.description || '';
        form.status.value = p.status || 'ACTIVE';
      });
    }
  });
}

function projectDetail(container, params) {
  renderPage(container, 'Project Detail', (body) => {
    api.getProject(params.id).then(resp => {
      const p = resp.data;
      body.innerHTML = `
        <div class="detail-grid">
          <div class="detail-label">ID</div><div class="detail-value">${p.id}</div>
          <div class="detail-label">Name</div><div class="detail-value">${p.name}</div>
          <div class="detail-label">Description</div><div class="detail-value">${p.description || '-'}</div>
          <div class="detail-label">Status</div><div class="detail-value">${statusBadge(p.status)}</div>
          <div class="detail-label">Created</div><div class="detail-value">${p.created_at}</div>
        </div>
        <div class="form-actions" style="margin-top:1rem">
          <button class="btn btn-primary" onclick="router.navigate('/projects/${p.id}/edit')">Edit</button>
          <button class="btn btn-danger" onclick="if(confirm('Delete?')){api.deleteProject(${p.id}).then(()=>router.navigate('/projects')).catch(e=>toast(e.message,'error'))}">Delete</button>
        </div>`;
    }).catch(e => toast(e.message, 'error'));
  });
}

// === Test Plans ===
function testPlanList(container) {
  renderPage(container, 'Test Plans', (body) => {
    const headerDiv = body.parentElement.querySelector('.page-header');
    headerDiv.appendChild(h('button', { class: 'btn btn-primary', onclick: () => router.navigate('/test-plans/new') }, '+ Add New'));
    const tableDiv = h('div'), pagDiv = h('div');
    body.appendChild(tableDiv); body.appendChild(pagDiv);

    function load(page = 1) {
      api.listTestPlans(page).then(resp => {
        tableDiv.innerHTML = '';
        tableDiv.appendChild(dataTable(['ID', 'Name', 'Version', 'Status', 'Updated'],
          resp.data.map(p => [p.id, `<a href="#/test-plans/${p.id}">${p.name}</a>`, p.version, statusBadge(p.status), p.updated_at?.split('T')[0] || '']),
          (td, row) => {
            td.appendChild(h('button', { class: 'btn-icon', title: 'Edit', onclick: () => router.navigate(`/test-plans/${row[0]}/edit`) }, '✏️'));
            td.appendChild(h('button', { class: 'btn-icon', title: 'Delete', onclick: () => { if (confirm('Delete?')) api.deleteTestPlan(row[0]).then(() => load(page)).catch(e => toast(e.message, 'error')); } }, '🗑️'));
          }));
        pagDiv.innerHTML = '';
        pagDiv.appendChild(pagination(page, resp.meta.total, resp.meta.limit, load));
      }).catch(e => toast(e.message, 'error'));
    }
    load();
  });
}

function testPlanForm(container, params) {
  const isEdit = !!params.id;
  renderPage(container, isEdit ? 'Edit Test Plan' : 'Create Test Plan', (body) => {
    const form = h('form', { class: 'form', onsubmit: async (e) => {
      e.preventDefault();
      const data = {
        name: form.name.value,
        project_ids: form.project_ids.value.split(',').map(s => parseInt(s.trim())).filter(n => n),
        types: form.types.value.split(',').map(s => s.trim()).filter(s => s),
        version: form.version.value || '1.0',
        description: form.description.value || null,
      };
      try {
        if (isEdit) await api.updateTestPlan(params.id, data);
        else await api.createTestPlan(data);
        toast(isEdit ? 'Updated' : 'Created', 'success');
        router.navigate('/test-plans');
      } catch (err) { toast(err.message, 'error'); }
    }});
    form.innerHTML = `
      <div class="form-group"><label>Name</label><input name="name" required /></div>
      <div class="form-group"><label>Project IDs (comma-separated)</label><input name="project_ids" placeholder="1,2,3" /></div>
      <div class="form-group"><label>Types (comma-separated)</label><input name="types" placeholder="REGRESSION, API" /></div>
      <div class="form-group"><label>Version</label><input name="version" value="1.0" /></div>
      <div class="form-group"><label>Description</label><textarea name="description"></textarea></div>
      <div class="form-actions">
        <button type="submit" class="btn btn-primary">${isEdit ? 'Update' : 'Create'}</button>
        <button type="button" class="btn" onclick="router.navigate('/test-plans')">Cancel</button>
      </div>`;
    body.appendChild(form);
  });
}

function testPlanDetail(container, params) {
  renderPage(container, 'Test Plan Detail', (body) => {
    api.getTestPlan(params.id).then(resp => {
      const p = resp.data;
      body.innerHTML = `
        <div class="detail-grid">
          <div class="detail-label">ID</div><div class="detail-value">${p.id}</div>
          <div class="detail-label">Name</div><div class="detail-value">${p.name}</div>
          <div class="detail-label">Version</div><div class="detail-value">${p.version}</div>
          <div class="detail-label">Status</div><div class="detail-value">${statusBadge(p.status)}</div>
          <div class="detail-label">Description</div><div class="detail-value">${p.description || '-'}</div>
          <div class="detail-label">Projects</div><div class="detail-value">${(p.project_ids || []).join(', ')}</div>
          <div class="detail-label">Created</div><div class="detail-value">${p.created_at}</div>
        </div>
        <div class="form-actions" style="margin-top:1rem">
          <button class="btn btn-primary" onclick="router.navigate('/test-plans/${p.id}/edit')">Edit</button>
          <button class="btn btn-danger" onclick="if(confirm('Delete?')){api.deleteTestPlan(${p.id}).then(()=>router.navigate('/test-plans')).catch(e=>toast(e.message,'error'))}">Delete</button>
          <select id="status-select" class="btn" style="margin-left:1rem"><option value="">Transition Status...</option><option value="IN_PROGRESS">IN_PROGRESS</option><option value="DONE">DONE</option><option value="CANCEL">CANCEL</option></select>
        </div>`;
      document.getElementById('status-select').addEventListener('change', function() {
        if (this.value) api.transitionPlanStatus(p.id, this.value).then(() => { toast('Status updated', 'success'); router.navigate(`/test-plans/${p.id}`); }).catch(e => toast(e.message, 'error'));
      });
    }).catch(e => toast(e.message, 'error'));
  });
}

// === Test Runs ===
function testRunList(container) {
  renderPage(container, 'Test Runs', (body) => {
    body.parentElement.querySelector('.page-header').appendChild(h('button', { class: 'btn btn-primary', onclick: () => router.navigate('/test-runs/new') }, '+ Add New'));
    const tableDiv = h('div'), pagDiv = h('div');
    body.appendChild(tableDiv); body.appendChild(pagDiv);
    function load(page = 1) {
      api.listTestRuns(1, page).then(resp => {
        tableDiv.innerHTML = '';
        tableDiv.appendChild(dataTable(['ID', 'Summary', 'Cases', 'Updated'],
          resp.data.map(r => [r.id, `<a href="#/test-runs/${r.id}">${r.summary}</a>`, r.case_count, r.updated_at?.split('T')[0] || '']),
          (td, row) => {
            td.appendChild(h('button', { class: 'btn-icon', title: 'Cases', onclick: () => router.navigate(`/test-runs/${row[0]}/cases`) }, '📋'));
            td.appendChild(h('button', { class: 'btn-icon', title: 'Stats', onclick: () => router.navigate(`/test-runs/${row[0]}/stats`) }, '📊'));
            td.appendChild(h('button', { class: 'btn-icon', title: 'Delete', onclick: () => { if (confirm('Delete?')) api.deleteTestRun(1, row[0]).then(() => load(page)).catch(e => toast(e.message, 'error')); } }, '🗑️'));
          }));
        pagDiv.innerHTML = '';
        pagDiv.appendChild(pagination(page, resp.meta.total, resp.meta.limit, load));
      }).catch(e => toast(e.message, 'error'));
    }
    load();
  });
}

function testRunForm(container) {
  renderPage(container, 'Create Test Run', (body) => {
    const form = h('form', { class: 'form', onsubmit: async (e) => {
      e.preventDefault();
      const data = { summary: form.summary.value, version: form.version.value || null, notes: form.notes.value || null, planned_start_date: form.planned_start.value || null, planned_end_date: form.planned_end.value || null };
      try {
        await api.createTestRun(1, data);
        toast('Created', 'success');
        router.navigate('/test-runs');
      } catch (err) { toast(err.message, 'error'); }
    }});
    form.innerHTML = `
      <div class="form-group"><label>Summary</label><input name="summary" required /></div>
      <div class="form-group"><label>Version</label><input name="version" /></div>
      <div class="form-group"><label>Notes</label><textarea name="notes"></textarea></div>
      <div class="form-group"><label>Planned Start</label><input name="planned_start" type="date" /></div>
      <div class="form-group"><label>Planned End</label><input name="planned_end" type="date" /></div>
      <div class="form-actions">
        <button type="submit" class="btn btn-primary">Create</button>
        <button type="button" class="btn" onclick="router.navigate('/test-runs')">Cancel</button>
      </div>`;
    body.appendChild(form);
  });
}

function testRunDetail(container, params) {
  renderPage(container, 'Test Run Detail', (body) => {
    api.getTestRun(1, params.id).then(resp => {
      const r = resp.data;
      body.innerHTML = `
        <div class="detail-grid">
          <div class="detail-label">ID</div><div class="detail-value">${r.id}</div>
          <div class="detail-label">Summary</div><div class="detail-value">${r.summary}</div>
          <div class="detail-label">Version</div><div class="detail-value">${r.version || '-'}</div>
          <div class="detail-label">Plan ID</div><div class="detail-value">${r.plan_id || '-'}</div>
          <div class="detail-label">Notes</div><div class="detail-value">${r.notes || '-'}</div>
          <div class="detail-label">Created</div><div class="detail-value">${r.created_at}</div>
        </div>
        <div class="form-actions" style="margin-top:1rem">
          <button class="btn" onclick="router.navigate('/test-runs/${r.id}/cases')">Manage Cases</button>
          <button class="btn" onclick="router.navigate('/test-runs/${r.id}/stats')">View Statistics</button>
        </div>`;
    }).catch(e => toast(e.message, 'error'));
  });
}

function testRunStats(container, params) {
  renderPage(container, 'Test Run Statistics', (body) => {
    api.runStatistics(params.id).then(resp => {
      const s = resp.data;
      body.innerHTML = `
        <div class="detail-grid">
          <div class="detail-label">Total</div><div class="detail-value">${s.total}</div>
          <div class="detail-label">NOT TESTED</div><div class="detail-value">${s.not_tested}</div>
          <div class="detail-label">IN PROGRESS</div><div class="detail-value">${s.in_progress}</div>
          <div class="detail-label">PASS</div><div class="detail-value">${s.pass}</div>
          <div class="detail-label">FAIL</div><div class="detail-value">${s.fail}</div>
          <div class="detail-label">WARNING</div><div class="detail-value">${s.warning}</div>
          <div class="detail-label">IGNORE</div><div class="detail-value">${s.ignore}</div>
        </div>`;
    }).catch(e => toast(e.message, 'error'));
  });
}

// === Test Executions ===
function testExecutionList(container) {
  renderPage(container, 'Test Executions', (body) => {
    body.parentElement.querySelector('.page-header').appendChild(h('button', { class: 'btn btn-primary', onclick: () => router.navigate('/test-executions/new') }, '+ Add New'));
    const tableDiv = h('div'), pagDiv = h('div');
    body.appendChild(tableDiv); body.appendChild(pagDiv);
    function load(page = 1) {
      api.listTestExecutions(page).then(resp => {
        tableDiv.innerHTML = '';
        tableDiv.appendChild(dataTable(['ID', 'Name', 'Test Run', 'Testers', 'Updated'],
          resp.data.map(e => [e.id, `<a href="#/test-executions/${e.id}">${e.name}</a>`, e.test_run_id, e.tester_count, e.updated_at?.split('T')[0] || '']),
          (td, row) => {}));
        pagDiv.innerHTML = '';
        pagDiv.appendChild(pagination(page, resp.meta.total, resp.meta.limit, load));
      }).catch(e => toast(e.message, 'error'));
    }
    load();
  });
}

function testExecutionForm(container) {
  renderPage(container, 'Create Test Execution', (body) => {
    const form = h('form', { class: 'form', onsubmit: async (e) => {
      e.preventDefault();
      const data = { name: form.name.value, test_run_id: parseInt(form.test_run_id.value) };
      try {
        await api.createTestExecution(data);
        toast('Created', 'success');
        router.navigate('/test-executions');
      } catch (err) { toast(err.message, 'error'); }
    }});
    form.innerHTML = `
      <div class="form-group"><label>Name</label><input name="name" required /></div>
      <div class="form-group"><label>Test Run ID</label><input name="test_run_id" type="number" required /></div>
      <div class="form-actions">
        <button type="submit" class="btn btn-primary">Create</button>
        <button type="button" class="btn" onclick="router.navigate('/test-executions')">Cancel</button>
      </div>`;
    body.appendChild(form);
  });
}

// === Users ===
function userList(container) {
  renderPage(container, 'Users', (body) => {
    const tableDiv = h('div'), pagDiv = h('div');
    body.appendChild(tableDiv); body.appendChild(pagDiv);
    function load(page = 1) {
      api.listUsers(page).then(resp => {
        tableDiv.innerHTML = '';
        tableDiv.appendChild(dataTable(['ID', 'Username', 'Email', 'Full Name', 'Status'],
          resp.data.map(u => [u.id, u.username, u.email, u.fullname, statusBadge(u.status)]),
          (td, row) => {}));
        pagDiv.innerHTML = '';
        pagDiv.appendChild(pagination(page, resp.meta.total, resp.meta.limit, load));
      }).catch(e => toast(e.message, 'error'));
    }
    load();
  });
}

// === Route Registration ===
router
  .on('/', (c) => { c.innerHTML = '<div class="page"><h1>HOA TCMS</h1><p>Select a section from the sidebar.</p></div>'; })
  .on('/login', loginPage)
  .on('/projects', projectList)
  .on('/projects/new', projectForm)
  .on('/projects/:id', projectDetail)
  .on('/projects/:id/edit', projectForm)
  .on('/test-plans', testPlanList)
  .on('/test-plans/new', testPlanForm)
  .on('/test-plans/:id', testPlanDetail)
  .on('/test-plans/:id/edit', testPlanForm)
  .on('/test-runs', testRunList)
  .on('/test-runs/new', testRunForm)
  .on('/test-runs/:id', testRunDetail)
  .on('/test-runs/:id/stats', testRunStats)
  .on('/test-executions', testExecutionList)
  .on('/test-executions/new', testExecutionForm)
  .on('/users', userList)
  .on('/test-cases', (c) => { renderPage(c, 'Test Cases', (b) => { b.innerHTML = '<p>Test case management — coming soon.</p>'; }); })
  .on('/groups', (c) => { renderPage(c, 'Groups', (b) => { b.innerHTML = '<p>Group management — coming soon.</p>'; }); })
  .on('/roles', (c) => { renderPage(c, 'Roles', (b) => { b.innerHTML = '<p>Role management — coming soon.</p>'; }); })
  .on('/permissions', (c) => { renderPage(c, 'Permissions', (b) => { b.innerHTML = '<p>Permission listing — coming soon.</p>'; }); })
  .on('/categories', (c) => { renderPage(c, 'Categories', (b) => { b.innerHTML = '<p>Category management — coming soon.</p>'; }); })
  .on('/templates', (c) => { renderPage(c, 'Templates', (b) => { b.innerHTML = '<p>Template management — coming soon.</p>'; }); });

// Start
router.start();
})();
