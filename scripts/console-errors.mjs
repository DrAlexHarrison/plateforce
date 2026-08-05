// `Log.entryAdded` stays silent for page console errors and handler throws, measured on the account check's page.
export function listenForConsoleErrors(socket) {
  const lines = [];
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(event.data);
    if (message.method === 'Log.entryAdded' && message.params.entry.level === 'error') {
      lines.push(message.params.entry.text);
    }
    if (message.method === 'Runtime.consoleAPICalled' && message.params.type === 'error') {
      lines.push(message.params.args.map((argument) => argument.value ?? argument.description).join(' '));
    }
    if (message.method === 'Runtime.exceptionThrown') {
      lines.push(message.params.exceptionDetails.exception?.description ?? message.params.exceptionDetails.text);
    }
  });
  return lines;
}
