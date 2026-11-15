package main

import (
	"crypto/aes"
	"crypto/cipher"
	"crypto/rand"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"sync"

	"tailscale.com/ipn"
)

// encStore is an ipn.StateStore kept in one AES-256-GCM encrypted file. The
// key comes from the app (OS keychain), so the node's private keys are never
// on disk in plain text.
type encStore struct {
	mu   sync.Mutex
	path string
	aead cipher.AEAD
	m    map[ipn.StateKey][]byte
}

const storeFile = "node-state.enc"

func openStore(dir string, key []byte) (*encStore, error) {
	block, err := aes.NewCipher(key)
	if err != nil {
		return nil, err
	}
	aead, err := cipher.NewGCM(block)
	if err != nil {
		return nil, err
	}
	s := &encStore{path: filepath.Join(dir, storeFile), aead: aead, m: map[ipn.StateKey][]byte{}}
	b, err := os.ReadFile(s.path)
	switch {
	case errors.Is(err, os.ErrNotExist):
		return s, nil
	case err != nil:
		return nil, err
	}
	n := aead.NonceSize()
	if len(b) < n {
		return nil, errors.New("state file is damaged")
	}
	plain, err := aead.Open(nil, b[:n], b[n:], []byte(storeFile))
	if err != nil {
		return nil, errors.New("state file can't be decrypted with the key in the keychain")
	}
	if err := json.Unmarshal(plain, &s.m); err != nil {
		return nil, err
	}
	return s, nil
}

func (s *encStore) ReadState(id ipn.StateKey) ([]byte, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	v, ok := s.m[id]
	if !ok {
		return nil, ipn.ErrStateNotExist
	}
	return append([]byte(nil), v...), nil
}

func (s *encStore) WriteState(id ipn.StateKey, bs []byte) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if bs == nil {
		delete(s.m, id)
	} else {
		s.m[id] = append([]byte(nil), bs...)
	}
	return s.save()
}

func (s *encStore) save() error {
	plain, err := json.Marshal(s.m)
	if err != nil {
		return err
	}
	nonce := make([]byte, s.aead.NonceSize())
	if _, err := rand.Read(nonce); err != nil {
		return err
	}
	sealed := s.aead.Seal(nonce, nonce, plain, []byte(storeFile))
	tmp := s.path + ".tmp"
	if err := os.WriteFile(tmp, sealed, 0o600); err != nil {
		return err
	}
	return os.Rename(tmp, s.path)
}
