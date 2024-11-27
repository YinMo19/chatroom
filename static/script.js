let roomListDiv = document.getElementById('room-list')
let messagesDiv = document.getElementById('messages')
let newMessageForm = document.getElementById('new-message')
let newRoomForm = document.getElementById('new-room')
let statusDiv = document.getElementById('status')

let roomTemplate = document.getElementById('room')
let messageTemplate = document.getElementById('message')

let messageField = newMessageForm.querySelector('#message')
let usernameField = newMessageForm.querySelector('#username')
let roomNameField = newRoomForm.querySelector('#name')

// import { marked } from 'marked';

var STATE = {
  room: 'Public',
  rooms: {},
  connected: false
}

// Generate a color from a "hash" of a string. Thanks, internet.
function hashColor (str) {
  let hash = 0
  for (var i = 0; i < str.length; i++) {
    hash = str.charCodeAt(i) + ((hash << 5) - hash)
    hash = hash & hash
  }

  return `hsl(${hash % 360}, 100%, 70%)`
}

// Add a new room `name` and change to it. Returns `true` if the room didn't
// already exist and false otherwise.
function addRoom (name) {
  if (STATE[name]) {
    // changeRoom(name);
    return false
  }

  var node = roomTemplate.content.cloneNode(true)
  var room = node.querySelector('.room')
  room.addEventListener('click', () => changeRoom(name))
  room.textContent = name
  room.dataset.name = name
  roomListDiv.appendChild(node)

  STATE[name] = []
  changeRoom(name)
  return true
}

function clearMessages () {
  // Clear the messages in the DOM
  messagesDiv.innerHTML = ''

  // Clear the messages in the state
  if (STATE[STATE.room]) {
    STATE[STATE.room] = []
  }
}

// Change the current room to `name`, restoring its messages.
function changeRoom (name) {
  fetch(`/history?room=${name}`)
    .then(response => response.json())
    .then(data => {
      clearMessages()
      data.forEach(msg => {
        // const utcDate = new Date(msg.created_at)
        // const options = {
        //   year: 'numeric',
        //   month: '2-digit',
        //   day: '2-digit',
        //   hour: '2-digit',
        //   minute: '2-digit',
        //   second: '2-digit',
        //   hour12: false,
        //   timeZone: Intl.DateTimeFormat().resolvedOptions().timeZone // 获取用户的时区
        // }
        // const formattedDate = new Intl.DateTimeFormat('zh-CN', options).format(utcDate)
        const utcDate = new Date(msg.created_at)
        const utc8Date = new Date(utcDate.getTime() + (16 * 60 * 60 * 1000))
        const formattedDate = utc8Date.toISOString().substring(0, 19) // 格式化为 "YYYY-MM-DD HH:MM:SS"
        // console.log('created_at:', utc8Date)
        // console.log('formattedDate:', formattedDate)
        addMessage(name, msg.username, msg.message, formattedDate, true)
      })
    })
    .catch(error => {
      console.error('Error fetching history messages:', error)
    })

  if (STATE.room == name) return

  var newRoom = roomListDiv.querySelector(`.room[data-name='${name}']`)
  var oldRoom = roomListDiv.querySelector(`.room[data-name='${STATE.room}']`)
  if (!newRoom || !oldRoom) return

  STATE.room = name
  oldRoom.classList.remove('active')
  newRoom.classList.add('active')

  messagesDiv.querySelectorAll('.message').forEach(msg => {
    messagesDiv.removeChild(msg)
  })

  // STATE[name].forEach((data) => addMessage(name, data.username, data.message))
  // Fetch history messages for the new room
}

// Add `message` from `username` to `room`. If `push`, then actually store the
// message. If the current room is `room`, render the message.
function addMessage (room, username, message, createdAt, push = false) {
  if (push) {
    STATE[room].push({ username, message })
  }

  if (STATE.room == room) {
    var node = messageTemplate.content.cloneNode(true)
    node.querySelector('.message .username').textContent = username
    node.querySelector('.message .username').style.color = hashColor(username)
    // node.querySelector(".message .text").textContent = message;
    node.querySelector('.message .text').innerHTML = marked.parse(message)
    node.querySelector('.message .time').textContent = createdAt // 设置时间元素的内容
    messagesDiv.appendChild(node)

    // // // Scroll to the bottom of the messages div
    // // // messagesDiv.scrollTop = messagesDiv.scrollHeight;
    // console.log('messagesDiv.clientHeight:', messagesDiv.clientHeight)
    // console.log('messagesDiv.scrollHeight:', messagesDiv.scrollHeight)
    // console.log('messagesDiv.scrollTop:', messagesDiv.scrollTop)
    // console.log('messagesDiv.scrollTop:', messagesDiv.scrollHeight - messagesDiv.clientHeight -
    //   messagesDiv.scrollTop + 1)

    // 延迟执行滚动操作
    const isScrolledToBottom =
      messagesDiv.scrollHeight - messagesDiv.clientHeight <=
      messagesDiv.scrollTop + 1000

    // If it is, scroll to the bottom after adding the message
    if (isScrolledToBottom) {
      setTimeout(() => {
        messagesDiv.scrollTop = messagesDiv.scrollHeight
      }, 0)
    }
  }
}

// Subscribe to the event source at `uri` with exponential backoff reconnect.
function subscribe (uri) {
  var retryTime = 1

  function connect (uri) {
    const events = new EventSource(uri)

    events.addEventListener('message', ev => {
      console.log('raw data', JSON.stringify(ev.data))
      console.log('decoded data', JSON.stringify(JSON.parse(ev.data)))
      const msg = JSON.parse(ev.data)
      if (
        !('message' in msg) ||
        !('room' in msg) ||
        !('username' in msg) ||
        !('created_at' in msg)
      )
        return
      addMessage(msg.room, msg.username, msg.message, msg.created_at, true)
    })

    events.addEventListener('open', () => {
      setConnectedStatus(true)
      console.log(`connected to event stream at ${uri}`)
      retryTime = 1
    })

    events.addEventListener('error', () => {
      setConnectedStatus(false)
      events.close()

      let timeout = retryTime
      retryTime = Math.min(64, retryTime * 2)
      console.log(`connection lost. attempting to reconnect in ${timeout}s`)
      setTimeout(() => connect(uri), (() => timeout * 1000)())
    })
  }

  connect(uri)
}

// Set the connection status: `true` for connected, `false` for disconnected.
function setConnectedStatus (status) {
  STATE.connected = status
  statusDiv.className = status ? 'connected' : 'reconnecting'
}

// Let's go! Initialize the world.
function init () {
  // Initialize some rooms.
  addRoom('Public')
  // changeRoom("Public");
  // addMessage('Public', 'Public(Admin)', '欢迎。发送消息吧~', true)

  // Set up the form handler.
  newMessageForm.addEventListener('submit', e => {
    e.preventDefault()

    const room = STATE.room
    const message = messageField.value
    const username = usernameField.value || 'guest'
    const createdAt =
      messageField.dataset.createdAt ||
      new Date().toLocaleString('zh-CN', { hour12: false }) // 默认值为当前时间，格式为 'YYYY-MM-DD HH:mm:ss'

    if (!message || !username) return

    // 验证 created_at 字段是否存在且格式正确
    // if (!createdAt || !isValidDateTime(createdAt)) {
    //   alert('created_at 字段不存在或格式不正确。请提供一个有效的日期时间字符串，格式为 YYYY-MM-DD HH:mm:ss。')
    //   return
    // }

    if (STATE.connected) {
      fetch('/message', {
        method: 'POST',
        body: new URLSearchParams({
          room,
          username,
          message,
          created_at: createdAt
        })
      }).then(response => {
        if (response.ok) messageField.value = ''
        setTimeout(() => {
          messagesDiv.scrollTop = messagesDiv.scrollHeight
        }, 0)
      })
    }
  })

  // Handle key press for Enter and Shift+Enter
  messageField.addEventListener('keydown', e => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      newMessageForm.dispatchEvent(new Event('submit'))
    }
  })

  // Set up the new room handler.
  newRoomForm.addEventListener('submit', e => {
    e.preventDefault()

    const room = roomNameField.value
    if (!room) return

    roomNameField.value = ''
    if (!addRoom(room)) return

    addMessage(
      room,
      'Rocket',
      `Look, your own "${room}" room! Nice.`,
      'Now',
      true
    )
  })

  // Subscribe to server-sent events.
  subscribe('/events')
}

// 验证日期时间字符串，格式为 YYYY-MM-DD HH:mm:ss
function isValidDateTime (dateString) {
  const regex = /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$/
  return regex.test(dateString) && !isNaN(Date.parse(dateString))
}

init()
