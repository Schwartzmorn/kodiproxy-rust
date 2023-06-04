import urllib.request


def _cec(args):
    url = 'http://localhost:{port}/cec/{action}'
    url = url.format(
        port=args.port, action='power-on' if args.action == 'on' else 'standby')
    if args.device is not None:
        url += '?device={}'.format(args.device)

    request = urllib.request.Request(url, method='GET')

    response = urllib.request.urlopen(request)

    print(response.read())


def _av_receiver(args):
    url = 'http://localhost:{port}/avreceiver/{action}?{param}={value}'
    if args.action in ['mute', 'unmute']:
        url = url.format(port=args.port, action='volume', param='mute',
                         value='true' if args.action == 'mute' else 'false')
    else:
        url = url.format(port=args.port, action='power',
                         param='power', value=args.action)

    request = urllib.request.Request(url, method='GET')

    response = urllib.request.urlopen(request)

    print(response.read())


def _kodi(args):
    url = 'http://localhost:{port}/jsonrpc/'

    data = {
        'jsonrpc': '2.0',
        'id': 1
    }
    if args.action in ['mute', 'unmute']:
        data.update('Application.SetMute', params={
            'mute': args.action == 'mute'})
    elif args.action in ['volume-decr', 'volume-incr']:
        data.update('Application.SetVolume', params={
            'volume': 'increment' if args.action == 'volume-incr' else 'decrement'})
    elif args.action == 'introspect':
        data.update(method='JSONRPC.Introspect')
    else:
        data.update(method='Application.Quit')

    request = urllib.request.Request(url, method='POST', data=data)

    response = urllib.request.urlopen(request)

    print(response.read())


if __name__ == '__main__':
    import argparse

    parser = argparse.ArgumentParser(
        'Kodi proxy tester', usage='Methods made to help interact with the kodi proxy')
    parser.add_argument('-p', '--port', type=int, default=8079,
                        help='Port where kodi proxy listens')

    subparsers = parser.add_subparsers(
        dest='cmd', help='part of the kodiproxy to interact with')
    subparsers.required = True

    subparser = subparsers.add_parser('av', help='method for the avreceiver')
    subparser.add_argument('action', choices=['mute', 'unmute', 'on', 'off'])
    subparser.set_defaults(func=_av_receiver)

    subparser = subparsers.add_parser('cec', help='method for the cec')
    subparser.add_argument('action', choices=['on', 'off'])
    subparser.add_argument('-d', '--device', default=None, type=int,
                           help='logical address of the CEC device (default: broadcast)')
    subparser.set_defaults(func=_cec)

    subparser = subparsers.add_parser('kodi', help='method for the cec')
    subparser.add_argument(
        'action', choices=['mute', 'unmute', 'volume-decr', 'volume-incr', 'off' 'introspect'])
    subparser.set_defaults(func=_kodi)

    args = parser.parse_args()
    args.func(args)
