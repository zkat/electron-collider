const { ipcRenderer, contextBridge } = require('electron')

contextBridge.exposeInMainWorld(
  'app',
  {
    setFullscreen: (flag) => ipcRenderer.invoke('setFullscreen', flag),
  }
)