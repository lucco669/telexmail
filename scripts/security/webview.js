// Executed in the real Tauri webview. No browser stand-in and no relaxed CSP.
(async () => {
  const {invoke} = window.__TAURI__.core;
  const nativeWindow = window.__TAURI__.window.getCurrentWindow();
  const assert = (condition, reason) => { if (!condition) throw new Error(reason); };
  window.__TELEX_PWNED = 0;
  const results = [];
  // Use the production frame attributes. Changes to the app's sandbox must change this test.
  const frame = document.createElement('iframe');
  frame.setAttribute('sandbox', 'allow-same-origin');
  frame.referrerPolicy = 'no-referrer';
  frame.title = 'Security regression fixture';
  document.body.replaceChildren(frame);
  frame.style.cssText = 'width:800px;height:500px';
  async function mount(document){
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('Mail frame did not load')), 10000);
      frame.onload = () => { clearTimeout(timer); resolve(); };
      frame.srcdoc = document;
    });
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  for (const fixture of window.TELEX_SECURITY_CORPUS){
    for (const dark of [null, {paper:'#14171b', ink:'#e2e6eb', link:'#6f9de6', line:'#2e343c', muted:'#9aa3ad'}]){
      const view = await invoke('get_html', {id:fixture.id, loadImages:false, dark});
      assert(view?.document, `${fixture.name}: no HTML view`);
      await mount(view.document);
      const doc = frame.contentDocument;
      assert(doc.body.textContent.includes('SAFE_CONTENT'), `${fixture.name}: valid content lost`);
      assert(!doc.querySelector('script,iframe,object,embed,form,base,svg,math'), `${fixture.name}: active element survived`);
      for (const element of doc.querySelectorAll('*')){
        assert([...element.attributes].every(a => !/^on/i.test(a.name)), `${fixture.name}: event handler survived`);
      }
      assert([...doc.images].every(i => !i.src || i.src.startsWith('data:')), `${fixture.name}: network image survived`);
      assert(window.__TELEX_PWNED === 0, `${fixture.name}: mail escaped into the application`);
      assert(!(await invoke('get_message', {id:fixture.id})).flagged, `${fixture.name}: mail accessed native IPC`);
      results.push({name:fixture.name, paper:dark ? 'dark' : 'light'});
    }
  }
  // Test the sandbox itself even if the sanitizer accidentally allowed a script in future.
  await mount('<script>parent.__TELEX_PWNED=1</script><img src=x onerror="parent.__TELEX_PWNED=1">');
  assert(window.__TELEX_PWNED === 0, 'The mail sandbox executed raw scripts');
  await nativeWindow.setTitle('TELEX_SECURITY_RESULT:' + JSON.stringify({status:'passed', cases:results.length, sandbox:true}));
})().catch(async error => {
  await window.__TAURI__.window.getCurrentWindow().setTitle('TELEX_SECURITY_ERROR:' + String(error));
});
