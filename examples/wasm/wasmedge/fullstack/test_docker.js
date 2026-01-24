#!/usr/bin/env node

/**
 * Docker test harness for fullstack WASM MCP server
 * Tests the MCP server running with HTTP transport
 * Mirrors tests from test_harness.sh for consistency
 */

const http = require('http');

// ANSI color codes
const RED = '\x1b[31m';
const GREEN = '\x1b[32m';
const YELLOW = '\x1b[33m';
const BLUE = '\x1b[34m';
const NC = '\x1b[0m';

// Test configuration - HTTP server endpoint
const MCP_HOST = process.env.MCP_SERVER_HOST || 'mcp-server';
const MCP_PORT = parseInt(process.env.MCP_SERVER_PORT || '8080');

let testsRun = 0;
let testsPassed = 0;
let testsFailed = 0;

// Session management
let sessionId = null;

/**
 * Parse SSE response to extract JSON data
 */
function parseSSEResponse(data) {
    // Look for JSON data in SSE format (lines starting with "data: {")
    const lines = data.split('\n');
    for (const line of lines) {
        if (line.startsWith('data: {')) {
            try {
                return JSON.parse(line.substring(6));
            } catch (e) {
                // Continue looking
            }
        }
    }
    // If no SSE format, try parsing as plain JSON
    try {
        return JSON.parse(data);
    } catch (e) {
        return null;
    }
}

/**
 * Send JSON-RPC request to MCP server via HTTP with SSE support
 */
function sendRequest(request, skipInit = false) {
    return new Promise((resolve, reject) => {
        // Send initialized notification first if we have a session and it's not an init request
        const sendInitNotification = () => {
            if (sessionId && request.method !== 'initialize' && !skipInit) {
                const notificationData = JSON.stringify({
                    jsonrpc: '2.0',
                    method: 'notifications/initialized',
                    params: {}
                });

                const initOptions = {
                    hostname: MCP_HOST,
                    port: MCP_PORT,
                    path: '/mcp',
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                        'Accept': 'text/event-stream, application/json',
                        'Content-Length': Buffer.byteLength(notificationData),
                        'Mcp-Session-Id': sessionId
                    },
                    timeout: 5000
                };

                const initReq = http.request(initOptions, (res) => {
                    let data = '';
                    res.on('data', (chunk) => data += chunk);
                    res.on('end', () => sendMainRequest());
                });

                initReq.on('error', reject);
                initReq.write(notificationData);
                initReq.end();
            } else {
                sendMainRequest();
            }
        };

        const sendMainRequest = () => {
            const postData = JSON.stringify(request);

            const options = {
                hostname: MCP_HOST,
                port: MCP_PORT,
                path: '/mcp',
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'Accept': 'text/event-stream, application/json',
                    'Content-Length': Buffer.byteLength(postData)
                },
                timeout: 5000
            };

            if (sessionId) {
                options.headers['Mcp-Session-Id'] = sessionId;
            }

            const req = http.request(options, (res) => {
                let data = '';

                // Extract session ID from headers
                if (res.headers['mcp-session-id']) {
                    sessionId = res.headers['mcp-session-id'];
                }

                res.on('data', (chunk) => {
                    data += chunk;
                });

                res.on('end', () => {
                    const result = parseSSEResponse(data);
                    if (result) {
                        resolve(result);
                    } else if (data.trim() === '') {
                        // Empty response - likely a policy violation
                        resolve({ error: { message: 'Empty response (policy violation)' } });
                    } else {
                        reject(new Error(`Invalid response format: ${data.substring(0, 100)}`));
                    }
                });
            });

            req.on('error', reject);
            req.on('timeout', () => {
                req.destroy();
                reject(new Error('Request timeout'));
            });

            req.write(postData);
            req.end();
        };

        sendInitNotification();
    });
}

/**
 * Run a test and track results
 */
async function runTest(name, testFunc, expectFailure = false) {
    testsRun++;
    console.log(`${YELLOW}Test ${testsRun}: ${name}${NC}`);

    try {
        const result = await testFunc();
        const passed = result;

        if (passed) {
            console.log(`${GREEN}   Pass${NC}`);
            testsPassed++;
        } else {
            console.log(`${RED}   Fail${NC}`);
            testsFailed++;
        }
        return passed;
    } catch (error) {
        if (expectFailure) {
            console.log(`${GREEN}   Pass (expected failure: ${error.message})${NC}`);
            testsPassed++;
            return true;
        } else {
            console.log(`${RED}   Error: ${error.message}${NC}`);
            testsFailed++;
            return false;
        }
    }
}

/**
 * Main test function
 */
async function runTests() {
    console.log(`${BLUE}=== Testing Fullstack MCP Server in Docker ===${NC}\n`);

    // Initialize connection
    await runTest('Initialize MCP connection', async () => {
        const response = await sendRequest({
            jsonrpc: '2.0',
            method: 'initialize',
            params: {
                protocolVersion: '2024-11-05',
                capabilities: {},
                clientInfo: { name: 'docker-test', version: '1.0.0' }
            },
            id: 1
        });
        return response && response.result;
    });

    // Test 1: List available tools
    await runTest('List available tools', async () => {
        const response = await sendRequest({
            jsonrpc: '2.0',
            method: 'tools/list',
            params: {},
            id: 2
        });

        if (response && response.result) {
            const tools = response.result.tools || [];
            console.log(`    Found ${tools.length} tools`);
            const expectedTools = ['test_connection', 'fetch_todos', 'create_todo',
                                  'update_todo', 'delete_todo', 'batch_process',
                                  'search_todos', 'db_stats', 'read_wal'];
            const toolNames = tools.map(t => t.name);
            const hasAllTools = expectedTools.every(tool => toolNames.includes(tool));
            if (!hasAllTools) {
                console.log(`    Missing tools: ${expectedTools.filter(t => !toolNames.includes(t))}`);
            }
            return tools.length >= 9; // We expect at least 9 tools
        }
        return false;
    });

    // Test 2: Test database connection
    await runTest('Test database connection', async () => {
        const response = await sendRequest({
            jsonrpc: '2.0',
            method: 'tools/call',
            params: {
                name: 'test_connection',
                arguments: {}
            },
            id: 3
        });
        return response && response.result;
    });

    // Test 3: Create todos for testing
    await runTest('Create first todo', async () => {
        const response = await sendRequest({
            jsonrpc: '2.0',
            method: 'tools/call',
            params: {
                name: 'create_todo',
                arguments: {
                    title: 'Test Todo 1',
                    user_id: 1
                }
            },
            id: 4
        });
        return response && response.result;
    });

    await runTest('Create second todo', async () => {
        const response = await sendRequest({
            jsonrpc: '2.0',
            method: 'tools/call',
            params: {
                name: 'create_todo',
                arguments: {
                    title: 'Test Todo 2',
                    user_id: 1
                }
            },
            id: 5
        });
        return response && response.result;
    });

    await runTest('Create third todo', async () => {
        const response = await sendRequest({
            jsonrpc: '2.0',
            method: 'tools/call',
            params: {
                name: 'create_todo',
                arguments: {
                    title: 'Test Todo 3',
                    user_id: 2
                }
            },
            id: 6
        });
        return response && response.result;
    });

    // Test 4: Fetch todos
    await runTest('Fetch all todos', async () => {
        const response = await sendRequest({
            jsonrpc: '2.0',
            method: 'tools/call',
            params: {
                name: 'fetch_todos',
                arguments: {}
            },
            id: 7
        });

        if (response && response.result) {
            const todos = response.result.todos || response.result || [];
            console.log(`    Found ${todos.length} todos`);
            return true;
        }
        return false;
    });

    await runTest('Fetch todos for user 1', async () => {
        const response = await sendRequest({
            jsonrpc: '2.0',
            method: 'tools/call',
            params: {
                name: 'fetch_todos',
                arguments: { user_id: 1 }
            },
            id: 8
        });
        return response && response.result;
    });

    // Test 5: Search todos
    await runTest('Search todos containing "Test"', async () => {
        const response = await sendRequest({
            jsonrpc: '2.0',
            method: 'tools/call',
            params: {
                name: 'search_todos',
                arguments: { title_contains: 'Test' }
            },
            id: 9
        });
        return response && response.result;
    });

    // Test 6: Update a todo
    await runTest('Update todo ID 1', async () => {
        const response = await sendRequest({
            jsonrpc: '2.0',
            method: 'tools/call',
            params: {
                name: 'update_todo',
                arguments: {
                    id: '1',
                    title: 'Updated Todo',
                    completed: true
                }
            },
            id: 10
        });
        return response && response.result;
    });

    // Test 7: Delete a todo
    await runTest('Delete todo ID 2', async () => {
        const response = await sendRequest({
            jsonrpc: '2.0',
            method: 'tools/call',
            params: {
                name: 'delete_todo',
                arguments: { id: '2' }
            },
            id: 11
        });
        return response && response.result;
    });

    // Test 8: Batch process todos
    await runTest('Batch complete todos', async () => {
        const response = await sendRequest({
            jsonrpc: '2.0',
            method: 'tools/call',
            params: {
                name: 'batch_process',
                arguments: {
                    operation: 'complete',
                    ids: ['1', '3']
                }
            },
            id: 12
        });
        return response && response.result;
    });

    // Test 9: Get database statistics
    await runTest('Get database statistics', async () => {
        const response = await sendRequest({
            jsonrpc: '2.0',
            method: 'tools/call',
            params: {
                name: 'db_stats',
                arguments: {}
            },
            id: 13
        });
        return response && response.result;
    });

    // Test 10: Read WAL stats
    await runTest('Read WAL statistics', async () => {
        const response = await sendRequest({
            jsonrpc: '2.0',
            method: 'tools/call',
            params: {
                name: 'read_wal',
                arguments: {}
            },
            id: 14
        });
        return response && response.result;
    });

    // ===== POLICY ENFORCEMENT TESTS =====
    console.log(`\n${BLUE}=== Policy Enforcement Tests ===${NC}`);

    // Test non-existent tool (should fail)
    await runTest('Call non-existent tool (should fail)', async () => {
        const response = await sendRequest({
            jsonrpc: '2.0',
            method: 'tools/call',
            params: {
                name: 'execute_shell',
                arguments: { cmd: 'ls' }
            },
            id: 15
        });
        return response && response.error; // Expect an error
    }, true);

    // Test invalid resource access (should fail)
    await runTest('Access forbidden resource (should fail)', async () => {
        const response = await sendRequest({
            jsonrpc: '2.0',
            method: 'resources/read',
            params: { uri: 'file:///etc/passwd' },
            id: 16
        });
        return response && response.error; // Expect an error
    }, true);

    // Test invalid tool arguments (should fail)
    await runTest('Invalid tool arguments (should fail)', async () => {
        const response = await sendRequest({
            jsonrpc: '2.0',
            method: 'tools/call',
            params: {
                name: 'create_todo',
                arguments: { invalid_field: 'test' } // Missing required fields
            },
            id: 17
        });
        return response && response.error; // Expect an error
    }, true);

    // Test missing required arguments (should fail)
    await runTest('Missing required arguments (should fail)', async () => {
        const response = await sendRequest({
            jsonrpc: '2.0',
            method: 'tools/call',
            params: {
                name: 'delete_todo',
                arguments: {} // Missing id field
            },
            id: 18
        });
        return response && response.error; // Expect an error
    }, true);

    // Print summary
    console.log(`\n${BLUE}=== Test Summary ===${NC}`);
    console.log(`Tests run: ${testsRun}`);
    console.log(`${GREEN}Tests passed: ${testsPassed}${NC}`);
    console.log(`${RED}Tests failed: ${testsFailed}${NC}`);

    if (testsFailed === 0) {
        console.log(`\n${GREEN}✓ All Docker tests passed!${NC}`);
        process.exit(0);
    } else {
        console.log(`\n${RED}ｘ Some Docker tests failed${NC}`);
        process.exit(1);
    }
}

// Wait a bit for server to be ready, then run tests
setTimeout(() => {
    runTests().catch(error => {
        console.error(`${RED}Test execution failed: ${error}${NC}`);
        process.exit(1);
    });
}, 2000);