package main

import (
	"log"
	"sync"
	"time"

	"github.com/fsnotify/fsnotify"
)

// Watcher monitors browser install directories for changes and triggers
// a callback after a debounce period. This mirrors the Swift app's
// FileWatcher class which uses DispatchSource with a 2-second debounce.
type Watcher struct {
	fsWatcher *fsnotify.Watcher
	onChange  func()
	debounce time.Duration
	mu       sync.Mutex
	timer    *time.Timer
}

// NewWatcher creates a file watcher that calls onChange when any of the
// watched browser directories are modified.
func NewWatcher(onChange func()) (*Watcher, error) {
	fsw, err := fsnotify.NewWatcher()
	if err != nil {
		return nil, err
	}

	w := &Watcher{
		fsWatcher: fsw,
		onChange:  onChange,
		debounce:  2 * time.Second,
	}

	go w.loop()

	// Watch all browser directories that exist
	for _, b := range browsers {
		if b.Installed() {
			if err := fsw.Add(b.InstallPath); err != nil {
				log.Printf("Warning: could not watch %s: %v", b.InstallPath, err)
			}
		}
	}

	return w, nil
}

func (w *Watcher) loop() {
	for {
		select {
		case event, ok := <-w.fsWatcher.Events:
			if !ok {
				return
			}
			if event.Has(fsnotify.Write) || event.Has(fsnotify.Create) ||
				event.Has(fsnotify.Remove) || event.Has(fsnotify.Rename) {
				w.scheduleCallback()
			}
		case err, ok := <-w.fsWatcher.Errors:
			if !ok {
				return
			}
			log.Printf("Watcher error: %v", err)
		}
	}
}

// scheduleCallback resets the debounce timer. The callback fires only after
// the debounce period elapses with no new events.
func (w *Watcher) scheduleCallback() {
	w.mu.Lock()
	defer w.mu.Unlock()

	if w.timer != nil {
		w.timer.Stop()
	}
	w.timer = time.AfterFunc(w.debounce, w.onChange)
}

// Close stops watching all directories.
func (w *Watcher) Close() error {
	return w.fsWatcher.Close()
}
