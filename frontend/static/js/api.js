// HOA TCMS — API client
const BASE = '/api/v1';

async function request(method, url, body) {
  const opts = { method, headers: { 'Accept': 'application/json' }, credentials: 'same-origin' };
  if (body && method !== 'GET') {
    opts.headers['Content-Type'] = 'application/json';
    opts.body = JSON.stringify(body);
  }
  const resp = await fetch(url, opts);
  if (resp.status === 204) return null;
  const data = await resp.json();
  if (!resp.ok) throw new Error(data?.error?.message || `HTTP ${resp.status}`);
  return data;
}

const api = {
  // Auth
  login: (body) => request('POST', `${BASE}/auth/login`, body),
  logout: () => request('POST', `${BASE}/auth/logout`),
  getMe: () => request('GET', `${BASE}/users/me`),

  // Projects
  listProjects: (page = 1, limit = 25) =>
    request('GET', `${BASE}/projects?page=${page}&limit=${limit}`),
  getProject: (id) => request('GET', `${BASE}/projects/${id}`),
  createProject: (body) => request('POST', `${BASE}/projects`, body),
  updateProject: (id, body) => request('PATCH', `${BASE}/projects/${id}`, body),
  deleteProject: (id) => request('DELETE', `${BASE}/projects/${id}`),
  listProjectMembers: (id) => request('GET', `${BASE}/projects/${id}/members`),

  // Test Plans
  listTestPlans: (page = 1, limit = 25) =>
    request('GET', `${BASE}/test-plans?page=${page}&limit=${limit}`),
  getTestPlan: (id) => request('GET', `${BASE}/test-plans/${id}`),
  createTestPlan: (body) => request('POST', `${BASE}/test-plans`, body),
  updateTestPlan: (id, body) => request('PATCH', `${BASE}/test-plans/${id}`, body),
  deleteTestPlan: (id) => request('DELETE', `${BASE}/test-plans/${id}`),
  selectTestPlans: () => request('GET', `${BASE}/test-plans/select`),
  transitionPlanStatus: (id, status) =>
    request('POST', `${BASE}/test-plans/${id}/transition-status`, { status }),

  // Test Runs
  listTestRuns: (projectId, page = 1, limit = 25) =>
    request('GET', `${BASE}/projects/${projectId}/test-runs?page=${page}&limit=${limit}`),
  getTestRun: (projectId, id) => request('GET', `${BASE}/projects/${projectId}/test-runs/${id}`),
  createTestRun: (projectId, body) => request('POST', `${BASE}/projects/${projectId}/test-runs`, body),
  updateTestRun: (projectId, id, body) =>
    request('PATCH', `${BASE}/projects/${projectId}/test-runs/${id}`, body),
  deleteTestRun: (projectId, id) =>
    request('DELETE', `${BASE}/projects/${projectId}/test-runs/${id}`),
  listRunCases: (id) => request('GET', `${BASE}/test-runs/${id}/cases`),
  manageRunCases: (id, body) => request('POST', `${BASE}/test-runs/${id}/cases`, body),
  runStatistics: (id) => request('GET', `${BASE}/test-runs/${id}/statistics`),

  // Test Executions
  listTestExecutions: (page = 1, limit = 25) =>
    request('GET', `${BASE}/test-executions?page=${page}&limit=${limit}`),
  getTestExecution: (id) => request('GET', `${BASE}/test-executions/${id}`),
  createTestExecution: (body) => request('POST', `${BASE}/test-executions`, body),
  updateTestExecution: (id, body) => request('PATCH', `${BASE}/test-executions/${id}`, body),
  deleteTestExecution: (id) => request('DELETE', `${BASE}/test-executions/${id}`),
  importCases: (id, caseIds) =>
    request('POST', `${BASE}/test-executions/${id}/import-cases`, { case_ids: caseIds }),

  // Test Case Files
  listFiles: (projectId, testCaseId) =>
    request('GET', `${BASE}/projects/${projectId}/test-cases/${testCaseId}/files`),
  deleteFile: (projectId, testCaseId, fileId) =>
    request('DELETE', `${BASE}/projects/${projectId}/test-cases/${testCaseId}/files/${fileId}`),

  // Test Case Results
  updateResult: (id, body) => request('PATCH', `${BASE}/test-case-results/${id}`, body),

  // Sharing
  listShares: (resourceType, resourceId) =>
    request('GET', `${BASE}/share/${resourceType}/${resourceId}`),
  createShare: (body) => request('POST', `${BASE}/share`, body),
  updateShare: (id, body) => request('PATCH', `${BASE}/share/${id}`, body),
  deleteShare: (id) => request('DELETE', `${BASE}/share/${id}`),

  // Users
  listUsers: (page = 1, limit = 25) =>
    request('GET', `${BASE}/users?page=${page}&limit=${limit}`),
  getUser: (id) => request('GET', `${BASE}/users/${id}`),
  createUser: (body) => request('POST', `${BASE}/users`, body),
  updateUser: (id, body) => request('PATCH', `${BASE}/users/${id}`, body),
  deleteUser: (id) => request('DELETE', `${BASE}/users/${id}`),

  // Health
  health: () => request('GET', `${BASE}/health`),
};
